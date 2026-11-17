# Third ladder: layer visibility, text, a sheet layer with a viewport, attributes by class,
# open and closed polygons. Millimetres. See 001_ladder.py for the helpers.

job.folder("ladder", "2027")
X = ("stl",)


def rect(x1, y1, x2, y2):
    vs.Rect(x1, y1, x2, y2)
    return vs.LNewObj()


def extrude(z0, z1, profile=lambda: vs.Rect(0, 500, 1000, 0)):
    vs.BeginXtrd(z0, z1)
    profile()
    vs.EndXtrd()
    return vs.LNewObj()


# --- layer visibility ---------------------------------------------------------------------


def second_layer(then):
    vs.Layer("Tweede laag")
    rect(0, 500, 1000, 0)
    then()
    vs.Layer("Ontwerplaag-1")
    rect(2000, 500, 3000, 0)


job.step("150_layer_shown", lambda: second_layer(lambda: None))
job.step("151_layer_hidden", lambda: second_layer(vs.HideLayer))
job.step("152_layer_gray", lambda: second_layer(vs.GrayLayer))

# --- text ---------------------------------------------------------------------------------


def text(x, y, words, size):
    vs.TextSize(size)
    vs.TextOrigin(x, y)
    vs.CreateText(words)
    return vs.LNewObj()


job.step("160_text", lambda: text(0, 0, "Hallo wereld", 12))
job.step("161_text_moved", lambda: text(1000, 500, "Hallo wereld", 12))
job.step("162_text_large", lambda: text(0, 0, "Hallo wereld", 24))
job.step("163_text_other", lambda: text(0, 0, "Tweede tekst", 12))
job.step("164_text_two_lines", lambda: text(0, 0, "Regel een" + chr(13) + "Regel twee", 12))

# --- a sheet layer with a viewport --------------------------------------------------------


def sheet():
    extrude(0, 3000)
    h = vs.CreateLayer("Blad-1", 2)
    job.log("sheet layer", h, "type", vs.GetObjectVariableInt(h, 154))
    vp = vs.CreateVP(h)
    job.log("viewport", vp, "type", vs.GetTypeN(vp))
    vs.UpdateVP(vp)
    vs.Layer("Ontwerplaag-1")


job.step("170_sheet_viewport", sheet)

# --- attributes by class ------------------------------------------------------------------


def by_class():
    vs.NameClass("PerKlasse")
    vs.SetClFillBack("PerKlasse", (65535, 0, 0))
    vs.SetClUseGraphic("PerKlasse", True)
    h = rect(0, 500, 1000, 0)
    vs.SetFillColorByClass(h)
    vs.NameClass("Geen")


job.step("180_fill_by_class", by_class)

# --- open and closed polygons -------------------------------------------------------------


def polygon(close):
    (vs.ClosePoly if close else vs.OpenPoly)()
    vs.BeginPoly()
    for x, y in [(0, 0), (1000, 0), (1000, 800), (0, 800)]:
        vs.AddPoint(x, y)
    vs.EndPoly()
    return vs.LNewObj()


job.step("190_polygon_closed", lambda: polygon(True))
job.step("191_polygon_open", lambda: polygon(False))
vs.ClosePoly()
