# Worksheet with two database rows (one per record format), text, real and integer fields,
# to see how the computed sub-rows are stored.

job.folder("ws")


def run():
    h = vs.GetObject("WSA")
    while h not in (None, 0):
        vs.DelObject(h)
        h = vs.GetObject("WSA")
    vs.Layer("Ontwerplaag-1")
    vs.NewField("WSRecA", "Tag", "", 4, 0)
    vs.NewField("WSRecA", "Num", "", 3, 0)
    vs.NewField("WSRecA", "Cnt", "", 1, 0)
    vs.NewField("WSRecB", "Tag", "", 4, 0)
    for i, (x, tag, num, cnt) in enumerate([(0, "alfa", 1.5, 7), (2000, "beta", 2, 8), (4000, "gamma", 3.25, 9)]):
        vs.Rect(x, 0, x + 1000, 600)
        o = vs.LNewObj()
        vs.SetRecord(o, "WSRecA")
        vs.SetRField(o, "WSRecA", "Tag", tag)
        vs.SetRField(o, "WSRecA", "Num", str(num))
        vs.SetRField(o, "WSRecA", "Cnt", str(cnt))
    for i, (x, tag) in enumerate([(0, "one"), (2000, "two")]):
        vs.Rect(x, 2000, x + 1000, 2600)
        o = vs.LNewObj()
        vs.SetRecord(o, "WSRecB")
        vs.SetRField(o, "WSRecB", "Tag", tag)
    ws = vs.CreateWS("WSA", 6, 4)
    f = lambda r, c, t: vs.SetWSCellFormula(ws, r, c, r, c, t)
    f(1, 1, "Tag"); f(1, 2, "Num"); f(1, 3, "Cnt"); f(1, 4, "Dbl")
    vs.SetWSCellFormula(ws, 2, 0, 2, 0, "=DATABASE((R IN ['WSRecA']))")
    f(2, 1, "=('WSRecA'.'Tag')"); f(2, 2, "=('WSRecA'.'Num')"); f(2, 3, "=('WSRecA'.'Cnt')"); f(2, 4, "=('WSRecA'.'Num')*2")
    f(3, 1, "total"); f(3, 2, "=SUM(B2)")
    vs.SetWSCellFormula(ws, 4, 0, 4, 0, "=DATABASE((R IN ['WSRecB']))")
    f(4, 1, "=('WSRecB'.'Tag')")
    vs.RecalculateWS(ws)
    for r in (2, 4):
        n = vs.GetWSSubrowCount(ws, r)
        job.log("row", r, "subrows", n)
        for s in range(1, n + 1):
            job.log("  ", s, [vs.GetWSSubrowCellStrN(ws, r, c, s) for c in range(1, 5)])
    job.log("size", vs.GetWSRowColumnCount(ws))
    job.save("560_ws_dbrows")


run()
