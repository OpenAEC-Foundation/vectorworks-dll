# Second ladder: a custom colour, opacity, mesh, polylines, rounded rectangle, NURBS, hidden
# class and layer, a scaled symbol, a saved view. Millimetres. See 001_ladder.py for the helpers.

job.folder("ladder", "2027")
X = ("stl",)


def line(x1, y1, x2, y2):
    vs.MoveTo(x1, y1)
    vs.LineTo(x2, y2)
    return vs.LNewObj()


def rect(x1, y1, x2, y2):
    vs.Rect(x1, y1, x2, y2)
    return vs.LNewObj()


def extrude(z0, z1, profile=lambda: vs.Rect(0, 500, 1000, 0)):
    vs.BeginXtrd(z0, z1)
    profile()
    vs.EndXtrd()
    return vs.LNewObj()


# --- colours and attributes --------------------------------------------------------------
job.step("100_rect_custom_fill", lambda: vs.SetFillBack(rect(0, 500, 1000, 0), (12345, 23456, 34567)))
job.log("index of custom colour", vs.RGBToColorIndex(12345, 23456, 34567))
job.step("101_rect_second_custom", lambda: vs.SetFillBack(rect(0, 500, 1000, 0), (4660, 22136, 39612)))
job.step("102_rect_no_fill", lambda: vs.SetFPat(rect(0, 500, 1000, 0), 0))
job.step("103_rect_opacity", lambda: vs.SetOpacity(rect(0, 500, 1000, 0), 40))
job.step("104_line_weight", lambda: vs.SetLW(line(0, 0, 1000, 0), 40))
job.step("105_extrude_red", lambda: vs.SetFillBack(extrude(0, 3000), (65535, 0, 0)), X)

# --- more 2D -----------------------------------------------------------------------------


def polyline_open():
    vs.OpenPoly()
    vs.BeginPoly()
    for p in [(0, 0), (1000, 0), (1000, 500)]:
        vs.AddPoint(p)
    vs.EndPoly()
    vs.ClosePoly()


def polyline_arcs():
    vs.ClosePoly()
    vs.BeginPoly()
    vs.AddPoint(0, 0)
    vs.ArcTo(1000, 0, 200)
    vs.AddPoint(1000, 800)
    vs.CurveTo(500, 1200)
    vs.AddPoint(0, 800)
    vs.EndPoly()


job.step("110_polygon_open", polyline_open)
job.step("111_polyline_arc_curve", polyline_arcs)
job.step("112_rounded_rect", lambda: vs.RRect(0, 500, 1000, 0, 200, 200))
job.step("113_extrude_polyline", lambda: extrude(0, 1000, polyline_arcs), X)

# --- more 3D -----------------------------------------------------------------------------


def mesh():
    vs.BeginMesh()
    for face in [
        [(0, 0, 0), (1000, 0, 0), (500, 500, 800)],
        [(1000, 0, 0), (1000, 1000, 0), (500, 500, 800)],
        [(1000, 1000, 0), (0, 1000, 0), (500, 500, 800)],
        [(0, 1000, 0), (0, 0, 0), (500, 500, 800)],
    ]:
        vs.BeginPoly3D()
        for p in face:
            vs.Add3DPt(p)
        vs.EndPoly3D()
    vs.EndMesh()


def nurbs_curve():
    h = vs.CreateNurbsCurve((0, 0, 0), True, 3)
    for p in [(1000, 0, 500), (1000, 1000, 500), (0, 1000, 1000), (0, 2000, 1000)]:
        vs.AddVertex3D(h, p)


job.step("120_mesh", mesh, X)
job.step("121_nurbs_curve", nurbs_curve)
job.step("122_nurbs_from_extrude", lambda: vs.ConvertToNURBS(extrude(0, 3000), False), X)
job.step("123_extrude_scaled", lambda: vs.HScale3D(extrude(0, 3000), 0, 0, 0, 2, 1, 0.5), X)
job.step("124_poly3d_five", lambda: (vs.BeginPoly3D(), [vs.Add3DPt(p) for p in [(0, 0, 0), (1000, 0, 0), (1200, 600, 300), (500, 900, 300), (-200, 600, 0)]], vs.EndPoly3D()), X)

# --- visibility ---------------------------------------------------------------------------


def hidden_class():
    vs.NameClass("Verborgen")
    line(0, 0, 1000, 0)
    vs.NameClass("Geen")
    line(0, 500, 1000, 500)
    vs.HideClass("Verborgen")


job.step("130_hidden_class", hidden_class)
vs.ShowClass("Verborgen")


def gray_class():
    vs.NameClass("Verborgen")
    line(0, 0, 1000, 0)
    vs.NameClass("Geen")
    vs.GrayClass("Verborgen")


job.step("131_gray_class", gray_class)
vs.ShowClass("Verborgen")


def class_colour():
    vs.NameClass("Gekleurd")
    rect(0, 500, 1000, 0)
    vs.NameClass("Geen")
    vs.SetClFillBack("Gekleurd", (0, 40000, 0))
    vs.SetClUseGraphic("Gekleurd", True)


job.step("132_class_colour", class_colour)

# --- a saved view ---------------------------------------------------------------------------


def saved_view():
    extrude(0, 3000)
    vs.SetView(60, 0, 45, 0, 0, 0)
    vs.VSave("Aanzicht A")
    vs.SetView(0, 0, 0, 0, 0, 0)


job.step("140_saved_view", saved_view)
