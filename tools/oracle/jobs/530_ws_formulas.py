# Worksheet formulas: one sheet with many formulas and literal kinds; the log gives, per cell,
# what the scripting interface reports (formula text, displayed string, value).

job.folder("ws")

FORMULAS = [
    "7", "-7", "70000", "123456789", "2.25", "-0.5", "Hello world", "'42", "",
    "=A1+A2", "=A1-A2", "=A1*A2", "=A1/A2", "=A1^2", "=-A1", "=(A1+A2)*A3",
    "=SUM(A1:A3)", "=SUM(A1:A3,5)", "=MAX(A1,A2)", "=MIN(A1:A3)", "=AVG(A1:A3)",
    "=ABS(-3)", "=SQRT(16)", "=IF(A1>5,1,2)", "=IF(A1=10,'yes','no')",
    "=A1>A2", "=A1<>A2", "=A1&B1", "=CONCAT(B1,B1)", "=LEN(B1)", "=UPPER(B1)",
    "=COUNT((C='Walls'))", "=COUNT((R IN ['WSRec']))", "=$A$1+B2", "=Sheet2!A1",
    "=AREA", "=PI", "=ROUND(2.567,1)", "=A1+1.5", "=100/3",
    "=('WSRec'.'Tag')", "=DATABASE((R IN ['WSRec']))", "=SUBTOTAL(A1:A3)",
]

ws = None
for name in ("WSA",):
    h = vs.GetObject(name)
    while h not in (None, 0):
        vs.DelObject(h)
        h = vs.GetObject(name)
ws = vs.CreateWS("WSA", len(FORMULAS) + 2, 3)
vs.NewField("WSRec", "Tag", "", 4, 0)
for i, (r, c, t) in enumerate([(1, 1, "10"), (2, 1, "20"), (3, 1, "30"), (1, 2, "Hi")]):
    vs.SetWSCellFormula(ws, r, c, r, c, t)
for i, f in enumerate(FORMULAS):
    r = i + 5
    vs.SetWSCellFormula(ws, r, 3, r, 3, f)
vs.RecalculateWS(ws)
job.log("size", vs.GetWSRowColumnCount(ws))
for r in range(1, len(FORMULAS) + 5):
    job.log(r, repr(vs.GetWSCellFormulaN(ws, r, 3)), "|", repr(vs.GetWSCellStringN(ws, r, 3)), "|", vs.GetWSCellValue(ws, r, 3), "| fmt", vs.GetWSCellNumberFormat(ws, r, 3))
job.save("530_ws_formulas")
