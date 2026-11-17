# Materials ladder 1: textures with a plain colour, their size, how objects, classes and wall
# sides point to them, and reflectivity and transparency shaders. Millimetres.

job.folder("mat")


def extrude(z0, z1, x=0):
    vs.BeginXtrd(z0, z1)
    vs.Rect(x, 500, x + 1000, 0)
    vs.EndXtrd()
    return vs.LNewObj()


def describe(t):
    """Logs a texture and its shader records, as the scripting interface sees them."""
    job.log("  texture", vs.GetName(t), "index", vs.Name2Index(vs.GetName(t)), "size in", vs.GetTextureSize(t))
    for family in (1, 2, 3, 4):
        s = vs.GetShaderRecord(t, family)
        if s in (None, 0):
            continue
        n = vs.NumFields(s)
        fields = []
        for k in range(1, n + 1):
            name = vs.GetFldName(s, k)
            fields.append((name, vs.GetRField(t, record_name(s), name)))
        job.log("   family", family, record_name(s), fields)


def record_name(s):
    try:
        return vs.GetName(vs.GetRecord(s, 0)) or vs.GetName(s)
    except Exception:
        return vs.GetName(s)


def plain(name, rgb, size_mm, extra=()):
    """A texture with a plain colour shader (family 1, prototype 40) of `rgb` (0..255)."""
    t = vs.CreateTexture()
    vs.SetName(t, name)
    s = vs.CreateShaderRecord(t, 1, 40)
    rec = record_name(s)
    job.log("  shader record", rec, "fields", [vs.GetFldName(s, k) for k in range(1, vs.NumFields(s) + 1)])
    for field, value in zip(("Color", "Color G", "Color B"), rgb):
        vs.SetRField(t, rec, field, str(value))
    for family, prototype, values in extra:
        e = vs.CreateShaderRecord(t, family, prototype)
        er = record_name(e)
        job.log("  shader record", er, "fields", [vs.GetFldName(e, k) for k in range(1, vs.NumFields(e) + 1)])
        for field, value in values:
            vs.SetRField(t, er, field, str(value))
    vs.SetTextureSize(t, size_mm / 25.4)
    describe(t)
    return t


made = []


def textured(rgb, size_mm, part=3, extra=()):
    def draw():
        t = plain("Tex", rgb, size_mm, extra)
        made.append(t)
        h = extrude(0, 1000)
        vs.SetTextureRefN(h, vs.Name2Index("Tex"), part, 0)
        job.log("  object texture", [(p, vs.GetTextureRefN(h, p, 0, False)) for p in (3, 4, 5, 6)])
    return draw


def cleanup():
    while made:
        vs.DelObject(made.pop())


job.step("201_tex_red", textured((200, 30, 40), 1000))
cleanup()
job.step("202_tex_green", textured((20, 180, 60), 1000))
cleanup()
job.step("203_tex_green_500", textured((20, 180, 60), 500))
cleanup()
job.step("204_tex_top", textured((20, 180, 60), 500, part=4))
cleanup()


def two_textures():
    a = plain("TexA", (250, 0, 0), 1000)
    b = plain("TexB", (0, 0, 250), 1000)
    made.extend([a, b])
    return vs.Name2Index("TexA"), vs.Name2Index("TexB")


def parts():
    a, b = two_textures()
    h = extrude(0, 1000)
    vs.SetTextureRefN(h, a, 4, 0)
    vs.SetTextureRefN(h, b, 6, 0)
    job.log("  parts", [(p, vs.GetTextureRefN(h, p, 0, False)) for p in (3, 4, 5, 6)])


job.step("205_tex_top_sides", parts)
cleanup()


def by_class():
    a, b = two_textures()
    vs.NameClass("Hout")
    vs.SetClTextureG("Hout", b)
    vs.SetClUseTexture("Hout", True)
    h = extrude(0, 1000)
    vs.SetTextureRefN(h, -1, 3, 0)
    job.log("  class texture", vs.GetClTextureG("Hout"), "object", vs.GetTextureRefN(h, 3, 0, False), "resolved", vs.GetTextureRefN(h, 3, 0, True))
    vs.NameClass("Geen")


job.step("206_tex_by_class", by_class)
cleanup()


def wall():
    a, b = two_textures()
    vs.Wall(0, 0, 5000, 0)
    h = vs.LNewObj()
    vs.SetTextureRefN(h, a, 7, 0)
    vs.SetTextureRefN(h, b, 8, 0)
    job.log("  wall", [(p, vs.GetTextureRefN(h, p, 0, False)) for p in (3, 7, 8, 9, 10, 11, 12, 13)])


job.step("207_tex_wall_sides", wall)
cleanup()

# reflectivity: plastic (family 2, prototype 35); transparency: plain (3, 10)
job.step("208_tex_plastic", textured((200, 30, 40), 1000, extra=((2, 35, ()),)))
cleanup()
job.step("209_tex_transparent", textured((200, 30, 40), 1000, extra=((3, 10, ()),)))
cleanup()
