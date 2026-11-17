# Worksheet formulas, second round: the Dutch argument separator is ';'. One formula per row in
# column 3; the log gives what the scripting interface reports for each.

job.folder("ws")

FORMULAS = [
    "=A1+A2", "=SUM(A1:A3)", "=SUM(A1:A3;5)", "=SUM(A1;A2)", "=MAX(A1;A2)", "=MIN(A1;A2;A3)", "=AVG(A1:A3)",
    "=IF(A1>5;1;2)", "=IF(A1=10;'yes';'no')", "=CONCAT(B1;B1)", "=ROUND(2.567;1)", "=A1&B1",
    "=A1=A2", "=A1<A2", "=A1>=A2", "=A1<=A2", "=A1<>A2", "=AND(A1>1;A2>1)", "=OR(A1>1;A2>1)", "=NOT(A1>1)",
    "=INT(2.5)", "=MOD(7;3)", "=SIN(1)", "=COS(1)", "=TAN(1)", "=ATAN(1)", "=LN(2)", "=LOG(2)", "=EXP(1)",
    "=LEFT(B1;1)", "=RIGHT(B1;1)", "=MID(B1;1;1)", "=LOWER(B1)", "=TRIM(B1)", "=VALUE('3')", "=TEXT(A1;'0')",
    "=COUNT(A1:A3)", "=COUNT((C='Walls'))", "=COUNT((R IN ['WSRec']))", "=COUNT((T=Rect))",
    "=COUNT((N='abc'))", "=COUNT(((C='Walls')&(R IN ['WSRec'])))", "=COUNT(((C='a')|(C='b')))",
    "=SUM((C='Walls');AREA)", "=SUM((R IN ['WSRec']);('WSRec'.'Num'))",
    "=('WSRec'.'Tag')", "=('WSRec'.'Num')", "=('WSRec'.'Num')*2", "=('WSRec'.'Tag')&'x'",
    "=DATABASE((R IN ['WSRec']))", "=DATABASE((C='Walls'))", "=DATABASE((T=Rect))", "=DATABASE(((R IN ['WSRec'])&(C='Walls')))",
    "=DATABASE((N='abc'))", "=DATABASE((L='Layer1'))", "=DATABASE((SEL))",
    "=AREA", "=PERIMETER", "=NAME", "=CLASS", "=LAYER", "=TYPE", "=VOLUME", "=LENGTH", "=HEIGHT", "=WIDTH", "=PI", "=TRUE", "=FALSE",
    "=ROW", "=COLUMN", "=NOW", "=TODAY", "=MAX(A1:A3)", "=(A1+A2)*A3", "=-A1", "=A1^2", "=$A$1", "=$A1", "=A$1",
    "=B1", "=B1+B1", "=C5", "=A100", "=Z5", "=A1:A3", "=100", "=1,5+2", "=1.5+2", "='abc'", "=1E3", "=A1%", "=SUM(A1:B3)",
    "=#OBJ", "=OBJ", "=ISBLANK(A1)", "=ISNUMBER(A1)", "=LEN(B1)", "=UPPER(B1)",
]

h = vs.GetObject("WSA")
while h not in (None, 0):
    vs.DelObject(h)
    h = vs.GetObject("WSA")
ws = vs.CreateWS("WSA", len(FORMULAS) + 4, 3)
vs.NewField("WSRec", "Tag", "", 4, 0)
vs.NewField("WSRec", "Num", "", 3, 0)
for r, c, t in [(1, 1, "10"), (2, 1, "20"), (3, 1, "30"), (1, 2, "Hi")]:
    vs.SetWSCellFormula(ws, r, c, r, c, t)
for i, f in enumerate(FORMULAS):
    r = i + 5
    vs.SetWSCellFormula(ws, r, 3, r, 3, f)
vs.RecalculateWS(ws)
job.log("size", vs.GetWSRowColumnCount(ws))
for i, f in enumerate(FORMULAS):
    r = i + 5
    job.log(r, repr(f), "->", repr(vs.GetWSCellFormulaN(ws, r, 3)), "|", repr(vs.GetWSCellStringN(ws, r, 3)))
job.save("550_ws_formulas2")
