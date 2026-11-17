# A NURBS curve on its own (type 111 as an object), a light (type 81) at a known point, and 3D
# polygons with and without fill and pen, to see what each keeps. Millimetres, on the first layer.

job.folder("cov")
vs.Layer("Ontwerplaag-1")


def curve():
    h = vs.CreateNurbsCurve((0, 0, 0), True, 2)
    vs.AddVertex3D(h, (1000, 1000, 500))
    vs.AddVertex3D(h, (2000, 0, 1000))
    job.log("curve", vs.GetTypeN(h), vs.NurbsCurveGetNumPieces(h), vs.NurbsGetNumPts(h, 0))
    job.log("points", [vs.NurbsGetPt3D(h, 0, k) for k in range(vs.NurbsGetNumPts(h, 0))])
    job.log("knots", [vs.NurbsKnot(h, 0, k) for k in range(vs.NurbsNumKnots(h, 0))])
    job.log("weights", [vs.NurbsGetWeight(h, 0, k) for k in range(vs.NurbsGetNumPts(h, 0))])


job.step("405_nurbs_curve", curve)


def light():
    try:
        h = vs.CreateLight((1000, 2000, 3000), 1, True, False)
    except Exception:
        h = vs.CreateLight(1000, 2000, 3000, 1, True, False)
    job.log("light", vs.GetTypeN(h), vs.GetLightLocation(h))


job.step("406_light", light)


def polygons():
    # a filled square (default attributes), one without fill, one without fill and pen
    for k, (fill, pen) in enumerate([(1, 2), (0, 2), (0, 0)]):
        vs.Poly3D((0, 2000 * k, 0), (1000, 2000 * k, 0), (1000, 2000 * k + 1000, 500), (0, 2000 * k + 1000, 500))
        h = vs.LNewObj()
        vs.SetFPat(h, fill)
        vs.SetLS(h, pen)
        job.log("poly", k, vs.GetFPat(h), vs.GetLS(h), vs.GetTypeN(h))
    # a 3D polygon of two points: a 3D line
    vs.Poly3D((0, 7000, 0), (3000, 7000, 1000))


job.step("407_poly3d_fill_pen", polygons, ("stl",))
