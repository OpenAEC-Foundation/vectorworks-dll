# Hatches small enough that Vectorworks' static hatch of them has many lines (logged), to pin
# down what a level's start, repeat, offset and dash factor mean; and a hatch made transparent.
# Millimetres.

job.folder("draw")

RED = vs.RGBToColorIndex(65535, 0, 0)


def hatch(name, page, levels):
    vs.BeginVectorFillN(name, page, False, 0)
    for level in levels:
        vs.AddVectorFillLayer(*level)
    vs.EndVectorFill()
    return vs.GetObject(name)


def dump(h, depth=1):
    t = vs.GetTypeN(h)
    if t == 2:
        a, b = vs.GetSegPt1(h), vs.GetSegPt2(h)
        job.log("  line", round(a[0], 3), round(a[1], 3), round(b[0], 3), round(b[1], 3), "lw", vs.GetLW(h), "pen", vs.GetPenFore(h))
    else:
        job.log("  type", t)
    if t == 11:
        c = vs.FInGroup(h)
        while c:
            dump(c, depth + 1)
            c = vs.NextObj(c)


hatch("S-een", False, [(0, 0, 2, 1, 0, 1.5, 0.5, 1, 255)])
hatch("S-twee", False, [(0.5, 0.25, 0, 3, 2, 0, 1.0, 1, 255), (0, 0, 3, 0, 0, 1, 0.6, 5, RED)])
hatch("S-pagina", True, [(0, 0, 2, 1, 0, 1.5, 0.5, 1, 255)])
t = hatch("S-transparant", False, [(0, 0, 1, 0, 0, 1, 1.0, 1, 255)])
try:
    vs.SetObjectVariableBoolean(t, 661, True)
    job.log("transparent now", vs.GetObjectVariableBoolean(t, 661))
except Exception as e:
    job.log("transparent failed", e)

made = []


def draw():
    for k, name in enumerate(["S-een", "S-twee", "S-pagina", "S-transparant"]):
        vs.Rect(300 * k, 100, 300 * k + 200, 0)
        h = vs.LNewObj()
        vs.SetVectorFill(h, name)
        made.append((h, name))
    # one with its fill turned and its axes twice as long
    vs.Rect(0, 400, 200, 300)
    h = vs.LNewObj()
    vs.SetVectorFill(h, "S-een")
    vs.SetObjectVariableReal(h, 74, 0.5)
    vs.SetObjectVariableReal(h, 112, 1016.0)
    vs.SetObjectVariableReal(h, 113, 1016.0)
    job.log("turned fill", [round(vs.GetObjectVariableReal(h, s), 4) for s in (70, 71, 72, 73, 74, 110, 111, 112, 113)])


job.step("172_hatch_lines", draw)
for h, name in made:
    for origin, angle in (((0, 0), 0), ((100, 50), 0), ((0, 0), 30)):
        job.log("static", name, "origin", origin, "angle", angle)
        try:
            s = vs.CreateStaticHatchFromObject(h, name, origin, angle)
            if s:
                dump(s)
                vs.DelObject(s)
        except Exception as e:
            job.log("  failed", e)
