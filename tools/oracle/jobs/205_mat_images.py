# Materials ladder 5: an image on the drawing (a bitmap) and a texture with an image, both from
# a small PNG this job writes itself (8 x 4 pixels, a colour per quarter). Millimetres.
#
# The first three samples were made. Vectorworks 2027 ended (the process was gone) during the
# last step, 263_texture_image: a texture bitmap made with CreateTextureBitmapD and given a paint
# node with the obsolete SetTexBitPaintNode. Do not run that step again; it is left here only
# as the record of what was tried. ImportImageFile ignored the point it was given (the bitmaps
# landed far away, at the same place for both points), see job 207.

import struct
import zlib

job.folder("mat")

PNG = os.path.join(job.folder("mat"), "quarters.png")


def write_png(path, width=8, height=4):
    rows = b""
    for y in range(height):
        rows += b"\x00"
        for x in range(width):
            left, top = x < width // 2, y < height // 2
            rgb = {(True, True): (255, 0, 0), (False, True): (0, 255, 0), (True, False): (0, 0, 255), (False, False): (255, 255, 0)}[(left, top)]
            rows += bytes(rgb)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))


write_png(PNG)


def bitmap(at=(100, 200)):
    def draw():
        h = vs.ImportImageFile(PNG, at)
        if h in (None, 0):
            h = vs.FSActLayer()
        job.log("  bitmap type", vs.GetTypeN(h), "box", vs.GetBBox(h), "width", vs.HWidth(h), "height", vs.HHeight(h), "pixels", vs.GetObjectVariableLongInt(h, 534), vs.GetObjectVariableLongInt(h, 535))
    return draw


job.step("260_bitmap", bitmap())
job.step("261_bitmap_moved", bitmap(at=(300, 200)))


def moved_scaled():
    bitmap()()
    h = vs.FSActLayer()
    vs.HScale2D(h, 100, 200, 2, 2, False)
    job.log("  scaled box", vs.GetBBox(h))


job.step("262_bitmap_scaled", moved_scaled)


def image_texture():
    h = vs.ImportImageFile(PNG, (0, 0))
    if h in (None, 0):
        h = vs.FSActLayer()
    job.log("  imported", vs.GetTypeN(h))
    paint = h if vs.GetTypeN(h) == 14 else vs.CreatePaintFromImage(h)
    t = vs.CreateTexture()
    vs.SetName(t, "TexImage")
    s = vs.CreateShaderRecord(t, 1, 41)
    b = vs.CreateTextureBitmapD(s)
    job.log("  shader", s, "bitmap", b, vs.GetTypeN(b) if b else None)
    try:
        vs.SetTexBitPaintNode(b, paint)
        job.log("  paint set")
    except Exception as e:
        job.log("  SetTexBitPaintNode failed", repr(e))
    vs.SetTextureBitmap(s, b)
    vs.SetTextureSize(t, 2000 / 25.4)
    vs.SetTexBitFeatureSize(b, 500 / 25.4)
    job.log("  texture size", vs.GetTextureSize(t), "feature", vs.GetTexBitFeatureSize(b), "start", vs.GetTexBFeatureStart(b), "end", vs.GetTexBFeatureEnd(b), "repeat", vs.GetTexBitRepHoriz(b), vs.GetTexBitRepVert(b), "bitmap of shader", vs.GetTextureBitmap(s))
    vs.DelObject(h)
    vs.BeginXtrd(0, 1000)
    vs.Rect(0, 500, 1000, 0)
    vs.EndXtrd()
    e = vs.LNewObj()
    vs.SetTextureRefN(e, vs.Name2Index("TexImage"), 3, 0)


# job.step("263_texture_image", image_texture)  # ended Vectorworks, see above
