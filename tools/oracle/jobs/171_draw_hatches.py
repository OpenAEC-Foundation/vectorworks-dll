# Hatches: definitions with known levels, objects filled with them, and the static hatch
# Vectorworks makes of each (its own lines, logged), to compare the reader's drawing with.
# Millimetres.

job.folder("draw")

RED = vs.RGBToColorIndex(65535, 0, 0)
BLUE = vs.RGBToColorIndex(0, 0, 65535)
GREEN = vs.RGBToColorIndex(0, 65535, 0)
job.log("colours", RED, BLUE, GREEN)


def hatch(name, page, wall, background, levels):
    vs.BeginVectorFillN(name, page, wall, background)
    for level in levels:
        vs.AddVectorFillLayer(*level)
    vs.EndVectorFill()
    h = vs.GetObject(name)
    job.log("hatch", name, "index", vs.Name2Index(name), "type", vs.GetTypeN(h), "levels", vs.GetObjectVariableInt(h, 660),
            "transparent", vs.GetObjectVariableBoolean(h, 661), "page", vs.GetObjectVariableBoolean(h, 662),
            "wall", vs.GetObjectVariableBoolean(h, 663), "symbol", vs.GetObjectVariableBoolean(h, 664))


def fill_space(h):
    return [round(vs.GetObjectVariableReal(h, s), 4) for s in (70, 71, 72, 73, 74, 110, 111, 112, 113)]


def dump(h, depth=1, budget=[0]):
    t = vs.GetTypeN(h)
    if t == 2:
        budget[0] += 1
        if budget[0] < 600:
            job.log("  " * depth, "line", vs.GetSegPt1(h), vs.GetSegPt2(h), "lw", vs.GetLW(h), "pen", vs.GetPenFore(h))
    else:
        job.log("  " * depth, "type", t)
    if t == 11:
        c = vs.FInGroup(h)
        while c:
            dump(c, depth + 1, budget)
            c = vs.NextObj(c)


hatch("H-een", False, False, 0, [(0, 0, 100, 100, 100, 0, 0.25, 7, 255)])
hatch("H-pagina", True, True, 5, [(10, 20, 30, 0, 0, 15, 1.0, 2, RED)])
hatch("H-drie", False, False, 0, [(2, 0, 75, 129.9, 75, -129.9, 0.4, 1, RED), (2, 0, -75, 129.9, -75, -129.9, 0.4, 1, BLUE), (2, 0, 0, 259.8, 75, -129.9, 0.25, 3, GREEN)])
hatch("H-wit", False, True, 1, [(0, 0, 0, 50, 50, 0, 1.0, 1, 255)])

made = {}


def rect(x0, y0, x1, y1, name):
    vs.Rect(x0, y1, x1, y0)
    h = vs.LNewObj()
    job.log("  set", name, vs.SetVectorFill(h, name), vs.GetVectorFill(h), "fpat", vs.GetFPat(h))
    return h


def draw():
    made["a"] = rect(0, 0, 1000, 500, "H-een")
    made["b"] = rect(2000, 0, 3000, 500, "H-pagina")
    made["c"] = rect(4000, 0, 5000, 1000, "H-drie")
    made["d"] = rect(0, 2000, 1000, 2500, "H-een")
    h = made["d"]
    # the same hatch moved and turned in its object
    vs.SetObjectVariableReal(h, 110, 100.0)
    vs.SetObjectVariableReal(h, 111, 50.0)
    vs.SetObjectVariableReal(h, 74, 0.5)
    made["e"] = rect(2000, 2000, 3000, 2500, "H-wit")
    for k in sorted(made):
        job.log("  fill space", k, fill_space(made[k]))


job.step("171_hatches", draw)
for k in sorted(made):
    h = made[k]
    ok, name = vs.GetVectorFill(h)
    for origin, angle in (((0, 0), 0), ((500, 250), 30)):
        job.log("static", k, name, "origin", origin, "angle", angle)
        try:
            s = vs.CreateStaticHatchFromObject(h, name, origin, angle)
            if s:
                dump(s)
                vs.DelObject(s)
        except Exception as e:
            job.log("  failed", e)
