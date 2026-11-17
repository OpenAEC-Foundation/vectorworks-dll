# Makes the sample ladder of docs/ORACLE.md inside Vectorworks.
#
# Run in Vectorworks: Tools > Plug-ins > Run Script, choose this file (Python).
# Set OUT to %VWX_SAMPLES%\ladder\<version> first; the folder must exist.
#
# DRAFT: the vs.* calls are checked against the Function Reference of the installed
# Vectorworks before the first run. Steps that return False below are made by hand.
#
# Every step starts from a cleared document in millimetres, draws, and saves the document as
# NN_name.vwx. Exports (IFC, DXF) are made afterwards from the saved files.

import vs

OUT = r"C:\vwx-samples\ladder\2026"


def clear():
    """Deletes every object on every layer and leaves one design layer."""
    vs.SelectAll()
    vs.DeleteObjs()


def save(name):
    vs.SaveActiveDocument(OUT + "\\" + name + ".vwx")


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


def extrude(x1, y1, x2, y2, z0, z1):
    vs.BeginXtrd(z0, z1)
    vs.Rect(x1, y1, x2, y2)
    vs.EndXtrd()
    return vs.LNewObj()


def s00():
    pass


def s01():
    pass


def s02():
    line(0, 0, 1000, 0)


def s03():
    line(0, 0, 2000, 0)


def s04():
    vs.SetName(line(0, 0, 1000, 0), "L1")


def s05():
    line(0, 0, 1000, 0)
    line(0, 500, 1000, 500)


def s06():
    rect(0, 0, 1000, 500)


def s07():
    polygon([(0, 0), (1000, 0), (1200, 600), (500, 900), (-200, 600)])


def s08():
    vs.ArcByCenter(0, 0, 500, 0, 360)
    vs.ArcByCenter(2000, 0, 500, 0, 90)


def s11():
    vs.NameClass("Muren")
    vs.NameClass("None")


def s12():
    vs.NameClass("Muren")
    h = line(0, 0, 1000, 0)
    vs.SetClass(h, "Muren")


def s13():
    h = rect(0, 0, 1000, 500)
    vs.SetFillBack(h, (65535, 0, 0))


def s14():
    extrude(0, 0, 1000, 500, 0, 3000)


def s15():
    extrude(0, 0, 1000, 500, 1000, 4000)


def s16():
    h = extrude(0, 0, 1000, 500, 0, 3000)
    vs.SetRot3D(h, 0, 0, 30, 0, 0, 0)


STEPS = [
    ("00_empty", s00),
    ("01_empty_again", s01),
    ("02_line", s02),
    ("03_line_longer", s03),
    ("04_line_named", s04),
    ("05_two_lines", s05),
    ("06_rectangle", s06),
    ("07_polygon", s07),
    ("08_circle_arc", s08),
    # 09, 10: layer name and second layer are made by hand (dialogs)
    ("11_class", s11),
    ("12_line_in_class", s12),
    ("13_rectangle_red", s13),
    ("14_extrude", s14),
    ("15_extrude_raised", s15),
    ("16_extrude_rotated", s16),
    # 17 and later: by hand until the calls are checked
]

for name, draw in STEPS:
    clear()
    draw()
    vs.ReDrawAll()
    save(name)

vs.AlrtDialog("Ladder saved to " + OUT)
