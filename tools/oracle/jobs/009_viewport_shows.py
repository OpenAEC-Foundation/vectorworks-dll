# Sixth ladder: what a viewport shows (layers, classes, a crop), saved one change at a time so
# that two files differ in that change only; a dashed line; a hatch. Millimetres.

job.folder("ladder", "2027")


def extrude(z0, z1, profile=lambda: vs.Rect(0, 500, 1000, 0)):
    vs.BeginXtrd(z0, z1)
    profile()
    vs.EndXtrd()
    return vs.LNewObj()


state = {}


def first():
    vs.Layer("Ontwerplaag-1")
    vs.NameClass("Verborgen")
    extrude(0, 3000)
    vs.NameClass("Geen")
    layer = vs.ActLayer()
    h = vs.GetObject("Blad-3")
    if h in (None, 0) or vs.GetTypeN(h) != 31:
        h = vs.CreateLayer("Blad-3", 2)
    vp = vs.CreateVP(h)
    vs.SetObjectVariableReal(vp, 1003, 50)
    vs.SetVPLayerVisibility(vp, layer, 0)
    vs.UpdateVP(vp)
    vs.Layer("Ontwerplaag-1")
    state.update(vp=vp, layer=layer, sheet=h)


def change(name, what):
    try:
        what(state["vp"], state["layer"])
        vs.UpdateVP(state["vp"])
        job.save(name)
    except Exception as e:
        job.log("FAILED", name, repr(e))


def crop(vp, layer):
    vs.Layer("Blad-3")
    vs.Rect(-10, 10, 10, -10)
    r = vs.LNewObj()
    job.log("  crop", vs.SetVPCropObject(vp, r))
    vs.Layer("Ontwerplaag-1")


job.step("240_vp_layer_visible", first)
change("241_vp_layer_invisible", lambda vp, layer: vs.SetVPLayerVisibility(vp, layer, -1))
change("242_vp_layer_gray", lambda vp, layer: vs.SetVPLayerVisibility(vp, layer, 2))
change("243_vp_layer_visible_again", lambda vp, layer: vs.SetVPLayerVisibility(vp, layer, 0))
change("244_vp_class_invisible", lambda vp, layer: job.log("  class", vs.SetVPClassVisibility(vp, "Verborgen", -1)))
change("245_vp_class_visible", lambda vp, layer: job.log("  class", vs.SetVPClassVisibility(vp, "Verborgen", 0)))
change("246_vp_crop", crop)

# --- a dashed line, a hatch ---------------------------------------------------------------


def line(x1, y1, x2, y2):
    vs.MoveTo(x1, y1)
    vs.LineTo(x2, y2)
    return vs.LNewObj()


def dashed():
    h = line(0, 0, 1000, 0)
    vs.SetLS(h, -2)
    job.log("  line style", vs.GetLS(h), vs.GetLSN(h))


job.step("250_line_plain", lambda: line(0, 0, 1000, 0))
job.step("251_line_dashed", dashed)


def hatched():
    vs.BeginVectorFillN("Arcering-45", False, False, 0)
    vs.AddVectorFillLayer(0, 0, 1, 1, 1, 0, 0.25, 1, 255)
    vs.EndVectorFill()
    vs.Rect(0, 500, 1000, 0)
    h = vs.LNewObj()
    job.log("  hatch", vs.SetVectorFill(h, "Arcering-45"), vs.GetVectorFill(h))


job.step("260_rect_hatch", hatched)
