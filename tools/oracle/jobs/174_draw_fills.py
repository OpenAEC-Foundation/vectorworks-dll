# Gradient and tile fills with known colours and geometry, each on a rectangle. Millimetres.

job.folder("draw")


def gradient():
    g = vs.CreateGradient("Verloop-A")
    try:
        job.log("  segments", vs.GetNumGradientSegments(g))
        vs.SetGradientData(g, 1, 0.0, 0.5, 255, 0, 0)
        vs.SetGradientData(g, 2, 1.0, 0.5, 0, 0, 255)
        job.log("  inserted", vs.InsertGradientSegment(g, 0.3, 0.4, 0, 255, 0))
        job.log("  segments", vs.GetNumGradientSegments(g), [vs.GetGradientData(g, k) for k in range(1, vs.GetNumGradientSegments(g) + 1)])
    except Exception as e:
        job.log("  gradient failed", e)
    job.log("gradient index", vs.Name2Index("Verloop-A"), "type", vs.GetTypeN(g))
    return vs.Name2Index("Verloop-A")


def tile():
    t = vs.CreateTile("Tegel-A")
    try:
        vs.MoveTo(0, 0)
        vs.LineTo(30, 20)
        job.log("  add line", vs.AddTileGeometryObject(t, vs.LNewObj()))
        vs.Rect(5, 15, 15, 5)
        job.log("  add rect", vs.AddTileGeometryObject(t, vs.LNewObj()))
        vs.SetTileOffsetPoint(t, (3, 4))
        vs.SetTileRepetitionPoint(t, (40, 25))
        vs.SetTileBackgroundColor(t, vs.RGBToColorIndex(0, 65535, 0))
        job.log("  tile offset", vs.GetTileOffsetPoint(t), "repeat", vs.GetTileRepetitionPoint(t), "background", vs.GetTileBackgroundColor(t))
    except Exception as e:
        job.log("  tile failed", e)
    job.log("tile index", vs.Name2Index("Tegel-A"), "type", vs.GetTypeN(t))
    return vs.Name2Index("Tegel-A")


def draw():
    g = gradient()
    t = tile()
    for k, index in enumerate([g, t]):
        vs.Rect(1500 * k, 500, 1500 * k + 1000, 0)
        h = vs.LNewObj()
        vs.SetFPat(h, -index)
        job.log("  rect", k, "fpat", vs.GetFPat(h), vs.GetVectorFill(h), "style", vs.GetObjectVariableLongInt(h, 695), "type", vs.GetObjectVariableInt(h, 696))


job.step("174_fills", draw)
