# Design layers that differ from a plain one in a single setting each, in one document: scale,
# base elevation and thickness, transparency, render mode, colours, cut plane, visibility.

job.folder("doc")


def layers():
    out = []
    h = vs.FLayer()
    while h:
        out.append(h)
        h = vs.NextLayer(h)
    return out


def make():
    for h in layers()[1:]:
        vs.DelObject(h)
    vs.Units(7)
    vs.SetOriginAbsolute(0, 0)

    def new(name):
        h = vs.CreateLayer(name, 1)
        vs.Layer(name)
        return h

    new("LP0")
    h = new("LPscale")
    vs.SetLScale(h, 50)
    h = new("LPelev")
    vs.SetLayerElevationN(h, 1000, 250)
    h = new("LPtrans")
    vs.SetLayerTransparency(40)
    h = new("LPrender")
    vs.SetLayerRenderMode(h, 3, False, False)
    h = new("LPpenfore")
    vs.LPenFore(5)
    h = new("LPfillfore")
    vs.LFillFore(7)
    h = new("LPcut")
    vs.SetLayerCutPlane(h, 1200)
    h = new("LPcutenable")
    vs.SetLayerEnableCutPl(h, True)
    h = new("LPzvals")
    vs.SetZVals(2000, 300)
    h = new("LPhidden")
    vs.HideLayer()
    h = new("LPgray")
    vs.GrayLayer()
    h = new("LPambient")
    vs.SetLayerAmbientColor(h, 65535, 32768, 0)
    for h in layers():
        n = vs.GetLName(h)
        job.log(n, "scale", vs.GetLScale(h), "elev", vs.GetLayerElevationN(h) if hasattr(vs, "GetLayerElevationN") else "?", "zvals", vs.GetZVals(h) if hasattr(vs, "GetZVals") else "?", "transp", vs.HGetLayerTransp(h), "render", vs.GetLayerRenderMode(h), "cut", vs.GetLayerCutPlane(h), vs.GetLayerEnableCutPl(h), "vis", vs.GetLVis(h))
    vs.Layer(vs.GetLName(layers()[0]))
    vs.Rect(0, 100, 100, 0)


job.step("606_layers", make)
