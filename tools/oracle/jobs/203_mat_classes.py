# Materials ladder 3: the texture slots of a class, a material, backgrounds. Millimetres.

job.folder("mat")

made = []


def plain(name, rgb):
    t = vs.CreateTexture()
    vs.SetName(t, name)
    vs.CreateShaderRecord(t, 1, 40)
    for field, value in zip(("Color", "Color G", "Color B"), rgb):
        vs.SetRField(t, "NNAMaxon Plain Color", field, str(value))
    made.append(t)
    return vs.Name2Index(name)


def cleanup():
    while made:
        try:
            vs.DelObject(made.pop())
        except Exception:
            pass


def class_slots():
    # one texture per slot, so that each slot shows its own reference number
    slots = ("G", "L", "R", "C", "T", "D")
    refs = {s: plain("Tex" + s, (10 * k, 0, 0)) for k, s in enumerate(slots)}
    vs.NameClass("Hout")
    vs.SetClUseTexture("Hout", True)
    for s in slots:
        getattr(vs, "SetClTexture" + s)("Hout", refs[s])
    job.log("  refs", refs, "class", {s: getattr(vs, "GetClTexture" + s)("Hout") for s in slots}, "use", vs.GetClUseTexture("Hout"))
    vs.Rect(0, 500, 1000, 0)
    vs.NameClass("Geen")


job.step("240_class_texture_slots", class_slots)
cleanup()


def class_use_off():
    ref = plain("TexG", (0, 200, 0))
    vs.NameClass("Hout")
    vs.SetClTextureG("Hout", ref)
    vs.SetClUseTexture("Hout", False)
    job.log("  class", vs.GetClTextureG("Hout"), "use", vs.GetClUseTexture("Hout"))
    vs.Rect(0, 500, 1000, 0)
    vs.NameClass("Geen")


job.step("241_class_texture_use_off", class_use_off)
cleanup()


def material():
    ref = plain("TexM", (0, 0, 200))
    m = vs.CreateMaterial("Mat1", True)
    made.append(m)
    ok, m = vs.SetMaterialTexture(m, ref)
    job.log("  material", vs.GetTypeN(m), vs.GetName(m), "texture", ok, vs.GetMaterialTexture(m), "tex ref", ref)
    vs.BeginXtrd(0, 1000)
    vs.Rect(0, 500, 1000, 0)
    vs.EndXtrd()
    h = vs.LNewObj()
    ok, h = vs.SetObjMaterialHandle(h, m)
    job.log("  object material", ok, vs.GetObjMaterialName(h), [(p, vs.GetTextureRefN(h, p, 0, False)) for p in (3, 4, 5, 6)])


job.step("242_material", material)
cleanup()


def background(kind):
    def draw():
        b = vs.CreateRenderBkg(kind)
        made.append(b)
        vs.SetName(b, "Bkg%d" % kind)
        job.log("  background", kind, vs.GetTypeN(b), vs.GetName(b), "index", vs.Name2Index("Bkg%d" % kind))
        for family in (5, 6):
            s = vs.GetShaderRecord(b, family)
            if s in (None, 0):
                continue
            rec = vs.GetName(vs.GetRecord(s, 0)) or vs.GetName(s)
            job.log("   family", family, rec, [(vs.GetFldName(s, k), vs.GetRField(b, rec, vs.GetFldName(s, k))) for k in range(1, vs.NumFields(s) + 1)])
        vs.Rect(0, 500, 1000, 0)
    return draw


job.step("243_background_one_colour", background(2))
cleanup()
job.step("244_background_two_colours", background(3))
cleanup()
job.step("245_background_clouds", background(1))
cleanup()
job.step("246_background_sky", background(4))
cleanup()
