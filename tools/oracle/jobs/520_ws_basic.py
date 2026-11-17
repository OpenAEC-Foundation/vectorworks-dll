# Worksheets: an empty one, one with known cells, one cell changed, formatting, a database row,
# and a worksheet image on a layer. Each sample holds exactly one worksheet "WSA".

job.folder("ws")


def fresh(rows=4, cols=3):
    for name in ("WSA", "WSB"):
        h = vs.GetObject(name)
        while h not in (None, 0):
            vs.DelObject(h)
            h = vs.GetObject(name)
    return vs.CreateWS("WSA", rows, cols)


def cell(ws, r, c, text):
    vs.SetWSCellFormula(ws, r, c, r, c, text)


def empty():
    fresh()


def filled(third="12345"):
    ws = fresh()
    cell(ws, 1, 1, "Alpha")
    cell(ws, 1, 2, "Beta")
    cell(ws, 1, 3, third)
    cell(ws, 2, 1, "Gamma")
    cell(ws, 2, 2, "3.5")
    cell(ws, 2, 3, "=C1*2")
    vs.RecalculateWS(ws)
    job.log("  size", vs.GetWSRowColumnCount(ws))
    return ws


def changed():
    filled("54321")


def formatted():
    ws = filled()
    vs.SetWSColumnWidth(ws, 1, 1, 150)
    vs.SetWSColumnWidth(ws, 2, 2, 90)
    vs.SetWSRowHeight(ws, 2, 2, 40, True, True)
    vs.SetWSCellTextFormat(ws, 1, 1, 1, 1, 0, 14, 1)
    vs.SetWSCellAlignment(ws, 1, 2, 1, 2, 3)
    vs.SetWSCellNumberFormat(ws, 2, 2, 2, 2, 1, 3)
    for c in (1, 2, 3):
        job.log("  width", c, vs.GetWSColumnWidth(ws, c))
    job.log("  height 2", vs.GetWSRowHeight(ws, 2))


def placed():
    ws = filled()
    vs.SetWSPlacement(ws, 130, 223, 698, 816)
    job.log("  placement", vs.GetWSPlacement(ws))


def image():
    ws = filled()
    vs.Layer("Ontwerplaag-1")
    img = vs.CreateWSImage(ws, 1000, 2000)
    job.log("  image", img, vs.GetTypeN(img), vs.GetBBox(img))


def database():
    ws = fresh(3, 3)
    vs.Layer("Ontwerplaag-1")
    vs.NewField("WSRec", "Tag", "", 4, 0)
    for i, x in enumerate((0, 2000, 4000)):
        vs.Rect(x, 0, x + 1000, 600)
        h = vs.LNewObj()
        vs.SetRecord(h, "WSRec")
        vs.SetRField(h, "WSRec", "Tag", "OBJ-%d" % (i + 1))
    cell(ws, 1, 1, "Tag")
    vs.SetWSCellFormula(ws, 2, 0, 2, 0, "=DATABASE((R IN ['WSRec']))")
    cell(ws, 2, 1, "=('WSRec'.'Tag')")
    vs.RecalculateWS(ws)
    job.log("  subrows", vs.GetWSSubrowCount(ws, 2))


job.step("520_ws_empty", empty)
job.step("521_ws_filled", filled)
job.step("522_ws_changed", changed)
job.step("523_ws_formatted", formatted)
job.step("524_ws_placed", placed)
job.step("525_ws_image", image)
job.step("526_ws_database", database)
