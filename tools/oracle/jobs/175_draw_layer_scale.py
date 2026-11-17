# A design layer at 1 : 25 with a page-based hatch (S-pagina of job 172, made again if missing)
# and a dashed line on it; the static hatch Vectorworks makes of the hatch there. Millimetres.

job.folder("draw")


def dump(h):
    t = vs.GetTypeN(h)
    if t == 2:
        a, b = vs.GetSegPt1(h), vs.GetSegPt2(h)
        job.log("  line", round(a[0], 3), round(a[1], 3), round(b[0], 3), round(b[1], 3))
    else:
        job.log("  type", t)
    if t == 11:
        c = vs.FInGroup(h)
        while c:
            dump(c)
            c = vs.NextObj(c)


if not vs.GetObject("P-pagina"):
    vs.BeginVectorFillN("P-pagina", True, False, 0)
    vs.AddVectorFillLayer(0, 0, 2, 1, 0, 1.5, 0.5, 1, 255)
    vs.EndVectorFill()
if not vs.GetObject("P-wereld"):
    vs.BeginVectorFillN("P-wereld", False, False, 0)
    vs.AddVectorFillLayer(0, 0, 2, 1, 0, 1.5, 0.5, 1, 255)
    vs.EndVectorFill()

made = []


def draw():
    layer = vs.ActLayer()
    vs.SetLScale(layer, 25)
    job.log("  layer scale", vs.GetLScale(layer))
    for k, name in enumerate(["P-pagina", "P-wereld"]):
        vs.Rect(3000 * k, 2000, 3000 * k + 2000, 0)
        h = vs.LNewObj()
        vs.SetVectorFill(h, name)
        made.append((h, name))


job.step("175_layer_scale_25", draw)
for h, name in made:
    job.log("static", name)
    try:
        s = vs.CreateStaticHatchFromObject(h, name, (0, 0), 0)
        if s:
            dump(s)
            vs.DelObject(s)
    except Exception as e:
        job.log("  failed", e)
vs.SetLScale(vs.ActLayer(), 1)
