# The first sample ladder (docs/ORACLE.md): each sample differs from a neighbour in one thing.
# Millimetres. Runs under tools/oracle/server.py (`vs` and `job` are in scope).

import math

job.folder("ladder", "2027")
vs.Units(7)  # millimetres
job.log("version", vs.GetVersionEx())


def line(x1, y1, x2, y2):
    vs.MoveTo(x1, y1)
    vs.LineTo(x2, y2)
    return vs.LNewObj()


def rect(x1, y1, x2, y2):
    vs.Rect(x1, y1, x2, y2)
    return vs.LNewObj()


def polygon(points):
    vs.BeginPoly()
    for x, y in points:
        vs.AddPoint(x, y)
    vs.EndPoly()
    return vs.LNewObj()


PENTAGON = [(0, 0), (1000, 0), (1200, 600), (500, 900), (-200, 600)]


def extrude(z0, z1, profile=lambda: vs.Rect(0, 500, 1000, 0)):
    vs.BeginXtrd(z0, z1)
    profile()
    vs.EndXtrd()
    return vs.LNewObj()


# --- document and 2D ---------------------------------------------------------------------
job.step("00_empty", lambda: None)
job.step("01_empty_again", lambda: None)
job.step("02_line", lambda: line(0, 0, 1000, 0))
job.step("03_line_end", lambda: line(0, 0, 2000, 500))
job.step("04_line_start", lambda: line(100, 200, 1000, 0))
job.step("05_line_named", lambda: vs.SetName(line(0, 0, 1000, 0), "L1"))
job.step("06_two_lines", lambda: (line(0, 0, 1000, 0), line(0, 500, 1000, 500)))
job.step("07_line_down", lambda: line(0, 500, 1000, 0))
job.step("10_rect", lambda: rect(0, 500, 1000, 0))
job.step("11_rect_moved", lambda: rect(200, 800, 1200, 300))
job.step(
    "12_rect_rotated",
    lambda: vs.RectangleN(0, 0, math.cos(math.radians(30)), math.sin(math.radians(30)), 1000, 500),
)
job.step("13_polygon5", lambda: polygon(PENTAGON))
job.step("14_polygon6", lambda: polygon(PENTAGON + [(-400, 300)]))
job.step("15_circle", lambda: vs.ArcByCenter(0, 0, 500, 0, 360))
job.step("16_arc", lambda: vs.ArcByCenter(0, 0, 500, 30, 90))
job.step("17_oval", lambda: vs.Oval(0, 500, 1000, 0))
job.step("18_locus", lambda: vs.Locus(100, 200))

# --- 3D ----------------------------------------------------------------------------------
X = ("ifc", "stl")
job.step("20_extrude", lambda: extrude(0, 3000), X)
job.step("21_extrude_high", lambda: extrude(0, 4000), X)
job.step("22_extrude_raised", lambda: extrude(1000, 4000), X)
job.step("23_extrude_rot_z30", lambda: vs.SetRot3D(extrude(0, 3000), 0, 0, 30, 0, 0, 0), X)
job.step("24_extrude_rot_x90", lambda: vs.SetRot3D(extrude(0, 3000), 90, 0, 0, 0, 0, 0), X)
job.step("25_extrude_moved", lambda: vs.Move3DObj(extrude(0, 3000), 2000, 3000, 0), X)
job.step("26_extrude_pentagon", lambda: extrude(0, 3000, lambda: polygon(PENTAGON)), X)
job.step("27_extrude_circle", lambda: extrude(0, 3000, lambda: vs.ArcByCenter(0, 0, 500, 0, 360)), X)


def mxtrd():
    vs.BeginMXtrd(0, 2000)
    vs.Rect(0, 1000, 1000, 0)
    vs.Rect(250, 750, 750, 250)
    vs.EndMXtrd()


def sweep():
    vs.BeginSweep(0, 360, 15, 0)
    vs.Rect(500, 300, 1000, 0)
    vs.EndSweep()


def poly3d():
    vs.BeginPoly3D()
    for p in [(0, 0, 0), (1000, 0, 0), (1000, 1000, 500), (0, 1000, 500)]:
        vs.Add3DPt(p)
    vs.EndPoly3D()


def two_boxes():
    a = extrude(0, 1000)
    b = extrude(500, 1500, lambda: vs.Rect(500, 750, 1500, 250))
    return a, b


job.step("28_multiple_extrude", mxtrd, X)
job.step("29_sweep", sweep, X)
job.step("30_poly3d", poly3d, X)
job.step("31_sphere", lambda: vs.CreateSphere((0, 0, 0), 500), X)
job.step("32_cone", lambda: vs.CreateCone((0, 0, 0), (0, 0, 1000), 500), X)
job.step("33_hemisphere", lambda: vs.CreateHemisphere((0, 0, 0), (0, 0, 500)), X)
job.step("34_solid_add", lambda: vs.AddSolid(*two_boxes()), X)
job.step("35_solid_subtract", lambda: vs.SubtractSolid(*two_boxes()), X)
job.step("36_locus3d", lambda: vs.Locus3D((100, 200, 300)))

# --- attributes and organisation ---------------------------------------------------------


def in_class():
    vs.NameClass("Muren")
    line(0, 0, 1000, 0)
    vs.NameClass("None")


def group():
    line(0, 0, 1000, 0)
    rect(0, 500, 1000, 0)
    vs.SelectAll()
    vs.Group()


job.step("40_line_in_class", in_class)
job.step("41_rect_fill_red", lambda: vs.SetFillBack(rect(0, 500, 1000, 0), (65535, 0, 0)))
job.step("42_line_pen_blue", lambda: vs.SetPenFore(line(0, 0, 1000, 0), (0, 0, 65535)))
job.step("43_group", group)
job.step("44_text", lambda: (vs.MoveTo(0, 0), vs.CreateText("Hallo VWX")))

# --- symbols -----------------------------------------------------------------------------


MADE = set()


def symbol(instances):
    # the definition stays in the document after the first sample
    if "Blok" not in MADE:
        MADE.add("Blok")
        vs.BeginSym("Blok")
        extrude(0, 1000)
        vs.EndSym()
    for x, y, angle in instances:
        vs.Symbol("Blok", x, y, angle)


job.step("50_symbol", lambda: symbol([(0, 0, 0)]), X)
job.step("51_symbol_two", lambda: symbol([(0, 0, 0), (3000, 1000, 45)]), X)

# --- building objects --------------------------------------------------------------------


def wall(x1, y1, x2, y2):
    vs.Wall(x1, y1, x2, y2)
    return vs.LNewObj()


def floor():
    vs.BeginFloor(200)
    vs.Rect(0, 4000, 5000, 0)
    vs.EndGroup()


def column():
    vs.BeginColumn(3000)
    vs.Rect(0, 300, 300, 0)
    vs.EndGroup()


job.step("60_wall", lambda: wall(0, 0, 5000, 0), X)
job.step("61_wall_long", lambda: wall(0, 0, 8000, 0), X)
job.step("62_wall_diagonal", lambda: wall(0, 0, 3000, 4000), X)
job.step("63_wall_thick", lambda: vs.SetWallThickness(wall(0, 0, 5000, 0), 300), X)
job.step("64_wall_heights", lambda: vs.SetWallHeights(wall(0, 0, 5000, 0), 2500, 3500), X)
job.step("65_two_walls", lambda: (wall(0, 0, 5000, 0), wall(5000, 0, 5000, 4000)), X)
job.step("70_floor", floor, X)
job.step("71_column", column, X)

# --- records -----------------------------------------------------------------------------


def record():
    vs.NewField("Gegevens", "Naam", "x", 4, 0)
    h = line(0, 0, 1000, 0)
    vs.SetRecord(h, "Gegevens")
    vs.SetRField(h, "Gegevens", "Naam", "Balk-1")


job.step("90_record", record)

# --- a second layer, last: the layer stays in the document ---------------------------------


def layer2():
    h = vs.CreateLayer("Verdieping 1", 1)
    vs.SetLayerElevation(h, 3000, 0)
    line(0, 0, 1000, 0)


job.step("95_layer2", layer2)
