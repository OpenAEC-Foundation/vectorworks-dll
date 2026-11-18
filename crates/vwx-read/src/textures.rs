//! Textures (type 97) and the colour each one gives what it covers in a rendered view.
//!
//! A texture keeps its shaders as records attached to it, one per shader, whose formats are
//! named after the shader (`NNAMaxon Plain Color`, `NNAMaxon Image Color`, ..): a plain colour
//! has its red, green and blue in the fields `Color`, `Color G` and `Color B`; an image colour
//! takes the image of the texture's bitmap (type 93, under one of its records) with a filter
//! colour `Filter_Color`, `Filter_Color G`, `Filter_Color B`. Each shader that uses an image
//! holds its own bitmap under its record (a displacement map besides the colour image of a
//! lawn): the colour image is the bitmap under the `Image Color` record. The image is a PNG or
//! JPEG file inside a record of type 119, from byte 234 on, after a header with its width,
//! height and bits per pixel and a tag `png ` or `jpeg` (stored reversed). What the texture
//! covers takes the average colour of the image times the filter.
//!
//! An object points to its texture by its reference number (a mesh or a 3D polygon at 0xcc, as
//! the crossed planes of a plant with its photo do; a plug-in object at
//! 0xec, a CSG solid at 0x17e), and a class at 0x9e (with five more parts of an object after
//! it, every 4 bytes: a roof class has its top and its edges there). In a garden design of 2027
//! the classes `Turf`, `Roadway`, `Roof-Main` point so to their grass, asphalt and roofing.

use std::collections::HashMap;

use vwx_core::container::Record;
use vwx_core::le;

use crate::raw::{kind, Raw};
use crate::records::{self, Value};

/// A texture.
pub const TEXTURE: u8 = 97;
/// The bitmap of a texture.
const BITMAP: u8 = 93;
/// An image: a PNG file after a header.
const IMAGE: u8 = 119;
/// How many pixels along each side of an image are averaged at most.
const SAMPLES: u32 = 64;

/// The colours of the textures of a document, looked up as objects need them.
#[derive(Default)]
pub struct Colours {
    known: HashMap<u32, Option<[u8; 3]>>,
}

impl Colours {
    /// The colour of the texture with reference number `texture` (none when it has no colour
    /// or image shader that is read, or when it is no texture).
    pub fn of(&mut self, raw: &Raw, texture: u32) -> Option<[u8; 3]> {
        if texture == 0 {
            return None;
        }
        *self.known.entry(texture).or_insert_with(|| raw.by_ref.get(&texture).filter(|&&i| raw.records[i].kind() == TEXTURE).and_then(|&i| colour(raw, i)))
    }

    /// The colour an object takes from its texture, or from that of its class.
    pub fn of_object(&mut self, raw: &Raw, r: &Record, class: Option<&Record>) -> Option<[u8; 3]> {
        if let Some(c) = object_texture(r).and_then(|t| self.of(raw, t)) {
            return Some(c);
        }
        let class = class?;
        // its texture for the whole of an object, else that of its first part that has one
        (0..6).filter_map(|k| le::u32(class.bytes, 0x9e + 4 * k)).find_map(|t| self.of(raw, t))
    }
}

/// The reference number of the texture an object has of its own.
fn object_texture(r: &Record) -> Option<u32> {
    let at = match r.kind() {
        kind::MESH | kind::POLYGON_3D => 0xcc,
        kind::PLUG_IN_OBJECT => 0xec,
        kind::CSG_SOLID => 0x17e,
        _ => return None,
    };
    le::u32(r.bytes, at).filter(|&t| t != 0)
}

/// The colour of texture record `i`: its plain colour, else the average of its image times its
/// filter colour.
pub fn colour(raw: &Raw, i: usize) -> Option<[u8; 3]> {
    let shaders = records::of_object(raw, i);
    let channel = |values: &[(String, Value)], name: &str| {
        values.iter().find(|v| v.0 == name).and_then(|v| match v.1 {
            Value::Integer(n) => Some(n.clamp(0, 255) as u8),
            Value::Number(x) if x.is_finite() => Some(x.clamp(0.0, 255.0) as u8),
            _ => None,
        })
    };
    let rgb = |values: &[(String, Value)], base: &str| Some([channel(values, base)?, channel(values, &format!("{base} G"))?, channel(values, &format!("{base} B"))?]);
    if let Some(c) = shaders.iter().filter(|s| s.0.ends_with("Plain Color")).find_map(|s| rgb(&s.1, "Color")) {
        return Some(c);
    }
    let (_, values) = shaders.iter().find(|s| s.0.ends_with("Image Color"))?;
    let filter = rgb(values, "Filter_Color").unwrap_or([255; 3]);
    let average = image(raw, i).and_then(average)?;
    Some(std::array::from_fn(|k| (u32::from(average[k]) * u32::from(filter[k]) / 255) as u8))
}

/// The image file (PNG or JPEG) of the colour shader of texture record `i`: the first image of
/// the bitmap under its `Image Color` record.
pub fn image<'a>(raw: &'a Raw, i: usize) -> Option<&'a [u8]> {
    let shader = raw.children[i].iter().copied().find(|&c| {
        let r = &raw.records[c];
        r.kind() == kind::RECORD && records::format_of(r).and_then(|f| raw.names.get(&f)).is_some_and(|n| n.ends_with("Image Color"))
    })?;
    let mut todo = vec![(shader, 0)];
    while let Some((c, depth)) = todo.pop() {
        if raw.records[c].kind() == BITMAP {
            let &image = raw.children[c].iter().find(|&&k| raw.records[k].kind() == IMAGE)?;
            let bytes = raw.records[image].bytes;
            let at = bytes.windows(4).position(|w| w == b"\x89PNG" || w[..3] == *b"\xff\xd8\xff")?;
            return Some(&bytes[at..]);
        }
        if depth < 3 {
            todo.extend(raw.children[c].iter().rev().map(|&k| (k, depth + 1)));
        }
    }
    None
}

/// The average colour of a PNG or JPEG image (its pixels weighted by their opacity), from at
/// most `SAMPLES` by `SAMPLES` of them.
pub fn average(file: &[u8]) -> Option<[u8; 3]> {
    if file.starts_with(b"\xff\xd8") {
        return average_jpeg(file);
    }
    average_png(file)
}

/// The average colour of a JPEG image, decoded at about `SAMPLES` pixels a side.
fn average_jpeg(file: &[u8]) -> Option<[u8; 3]> {
    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(file));
    decoder.scale(SAMPLES as u16, SAMPLES as u16).ok()?;
    let pixels = decoder.decode().ok()?;
    let channels = match decoder.info()?.pixel_format {
        jpeg_decoder::PixelFormat::L8 => 1,
        jpeg_decoder::PixelFormat::RGB24 => 3,
        // (CMYK and 16-bit grey are rare in textures)
        _ => return None,
    };
    let (mut sum, mut n) = ([0u64; 3], 0u64);
    for p in pixels.chunks_exact(channels) {
        for (k, s) in sum.iter_mut().enumerate() {
            *s += u64::from(p[k.min(channels - 1)]);
        }
        n += 1;
    }
    (n > 0).then(|| sum.map(|s| (s / n) as u8))
}

/// The average colour of a PNG image.
fn average_png(png_file: &[u8]) -> Option<[u8; 3]> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buffer).ok()?;
    let (width, height) = (frame.width, frame.height);
    let channels = frame.color_type.samples();
    let step_x = (width / SAMPLES).max(1);
    let step_y = (height / SAMPLES).max(1);
    let (mut sum, mut weight) = ([0f64; 3], 0f64);
    for y in (0..height).step_by(step_y as usize) {
        for x in (0..width).step_by(step_x as usize) {
            let at = (y as usize * width as usize + x as usize) * channels;
            let p = &buffer[at..at + channels];
            let (rgb, alpha) = match channels {
                1 => ([p[0]; 3], 255),
                2 => ([p[0]; 3], p[1]),
                3 => ([p[0], p[1], p[2]], 255),
                _ => ([p[0], p[1], p[2]], p[3]),
            };
            let a = f64::from(alpha) / 255.0;
            for k in 0..3 {
                sum[k] += f64::from(rgb[k]) * a;
            }
            weight += a;
        }
    }
    (weight > 0.0).then(|| sum.map(|s| (s / weight).round().clamp(0.0, 255.0) as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PNG of 2 x 1 pixels, red and blue, made by the encoder of the same crate.
    fn two_pixels() -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, 2, 1);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[255, 0, 0, 0, 0, 255]).unwrap();
        }
        out
    }

    #[test]
    fn the_average_of_an_image_is_that_of_its_pixels() {
        assert_eq!(average(&two_pixels()), Some([128, 0, 128]));
        assert_eq!(average(b"not a png"), None);
    }
}
