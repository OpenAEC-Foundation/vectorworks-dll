# Worksheet cell formatting: sample A has "x" in every cell (defaults), sample B changes one
# property per cell, so a diff of the cell records shows where each property is stored.

job.folder("ws")


def fresh(name, rows, cols):
    h = vs.GetObject("WSA")
    while h not in (None, 0):
        vs.DelObject(h)
        h = vs.GetObject("WSA")
    ws = vs.CreateWS("WSA", rows, cols)
    for r in range(1, rows + 1):
        for c in range(1, cols + 1):
            vs.SetWSCellFormula(ws, r, c, r, c, "x")
    return ws


def base():
    fresh("A", 6, 4)


def variants():
    ws = fresh("B", 6, 4)
    one = lambda r, c: (ws, r, c, r, c)
    vs.SetWSCellTextFormat(*one(1, 1), 3, 10, 0)       # font index 3
    vs.SetWSCellTextFormat(*one(1, 2), 0, 10, 2)       # italic
    vs.SetWSCellTextFormat(*one(1, 3), 0, 10, 4)       # underline
    vs.SetWSCellTextFormat(*one(1, 4), 0, 18, 0)       # size 18
    vs.SetWSCellAlignment(*one(2, 1), 1)
    vs.SetWSCellAlignment(*one(2, 2), 2)
    vs.SetWSCellAlignment(*one(2, 3), 3)
    vs.SetWSCellAlignment(*one(2, 4), 4)
    vs.SetWSCellVertAlignment(*one(3, 1), 1)
    vs.SetWSCellVertAlignment(*one(3, 2), 2)
    vs.SetWSCellVertAlignment(*one(3, 3), 3)
    vs.SetWSCellNumberFormat(*one(3, 4), 2, 4, "L", "T")
    vs.SetWSCellNumberFormat(*one(4, 1), 5, 2, "", "")
    vs.SetWSCellTextColor(*one(4, 2), 1237)
    vs.SetWSCellFill(*one(4, 3), 1, 256, 1237, 1)
    vs.SetWSCellWrapTextFlag(*one(4, 4), True)
    vs.SetWSCellBorders(ws, 5, 1, 5, 1, True, False, False, False, 0)
    vs.SetWSCellBorders(ws, 5, 2, 5, 2, False, True, False, False, 0)
    vs.SetWSCellBorders(ws, 5, 3, 5, 3, False, False, True, False, 0)
    vs.SetWSCellBorders(ws, 5, 4, 5, 4, False, False, False, True, 0)
    vs.SetWSTextAngle(*one(6, 1), 90)
    vs.WorksheetMergeCells(ws, 6, 2, 6, 3)
    vs.SetWSColumnWidth(ws, 4, 4, 200)
    vs.SetWSRowHeight(ws, 6, 6, 50, True, False)
    vs.RecalculateWS(ws)
    for r in range(1, 7):
        for c in range(1, 5):
            job.log(r, c, vs.GetWSCellTextFormat(ws, r, c), vs.GetWSCellAlignment(ws, r, c), vs.GetWSCellVertAlignment(ws, r, c),
                    vs.GetWSCellNumberFormat(ws, r, c), vs.GetWSCellTextColor(ws, r, c), vs.GetWSCellFill(ws, r, c),
                    vs.GetWSCellWrapTextFlag(ws, r, c), vs.GetWSCellBorder(ws, r, c) if False else "")


job.step("540_ws_fmt_base", base)
job.step("541_ws_fmt_vars", variants)
