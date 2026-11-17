# Fourth ladder: viewports on a sheet layer (scale, place, view, what they show), dimensions.
# Millimetres. See 001_ladder.py for the helpers.

job.folder("ladder", "2027")


def extrude(z0, z1, profile=lambda: vs.Rect(0, 500, 1000, 0)):
    vs.BeginXtrd(z0, z1)
    profile()
    vs.EndXtrd()
    return vs.LNewObj()


# --- viewports ----------------------------------------------------------------------------


def viewport(scale=50, then=lambda vp, layer: None, sheet="Blad-2"):
    vs.Layer("Ontwerplaag-1")
    extrude(0, 3000)
    layer = vs.ActLayer()
    h = vs.GetObject(sheet)
    if h in (None, 0) or vs.GetTypeN(h) != 31:
        h = vs.CreateLayer(sheet, 2)
    vp = vs.CreateVP(h)
    vs.SetVPLayerVisibility(vp, layer, 0)
    vs.SetObjectVariableReal(vp, 1003, scale)
    then(vp, layer)
    vs.UpdateVP(vp)
    job.log("  viewport scale", vs.GetObjectVariableReal(vp, 1003), "view", vs.GetObjectVariableInt(vp, 1007), "projection", vs.GetObjectVariableInt(vp, 1000),
            "render", vs.GetObjectVariableInt(vp, 1001), "at", vs.GetObjectVariableReal(vp, 1024), vs.GetObjectVariableReal(vp, 1025), "box", vs.GetBBox(vp))
    vs.Layer("Ontwerplaag-1")
    return vp


def moved(vp, layer):
    vs.SetObjectVariableReal(vp, 1024, 200)
    vs.SetObjectVariableReal(vp, 1025, 100)


job.step("200_vp_scale_50", lambda: viewport(50))
job.step("201_vp_scale_100", lambda: viewport(100))
job.step("202_vp_moved", lambda: viewport(50, moved))
for number, view in ((203, 3), (204, 6), (205, 9)):
    job.step("%d_vp_view_%d" % (number, view), lambda view=view: viewport(50, lambda vp, layer: vs.SetObjectVariableInt(vp, 1007, view)))
job.step("206_vp_layer_hidden", lambda: viewport(50, lambda vp, layer: vs.SetVPLayerVisibility(vp, layer, -1)))
job.step("207_vp_two", lambda: (viewport(50), viewport(100, moved)))

# --- dimensions ---------------------------------------------------------------------------


def dimension(x2, offset):
    vs.LinearDim((0, 0), (x2, 0), offset, 0, 771, 770, 0)
    return vs.LNewObj()


job.step("210_dimension", lambda: dimension(1000, 300))
job.step("211_dimension_longer", lambda: dimension(2000, 300))
job.step("212_dimension_further", lambda: dimension(1000, 600))
