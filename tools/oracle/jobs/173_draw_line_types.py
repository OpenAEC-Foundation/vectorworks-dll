# Line types (dash styles) with known dashes and gaps, on lines and a rectangle; what the
# scripting API reports of each. Millimetres.

job.folder("draw")


def report(name):
    index = vs.Name2Index(name)
    try:
        n, swt = vs.GetNumDashDataPairsN(-index)
        pairs = [vs.GetDashDataValPrAtN(-index, k) for k in range(1, n + 1)]
    except Exception as e:
        n, swt, pairs = "?", "?", str(e)
    job.log("line type", name, "index", index, "type", vs.GetTypeN(vs.GetObject(name)), "pairs", n, "swt", swt, pairs)
    return index


def make(name, swt, *values):
    try:
        vs.SetDashStyleN(name, swt, len(values) // 2, *values)
    except Exception as e:
        job.log("SetDashStyleN failed", name, e)
    return report(name)


def draw():
    a = make("LT-a", False, 10.0, 5.0)
    b = make("LT-b", True, 20.0, 5.0, 2.0, 5.0)
    c = make("LT-c", False, 0.5, 0.25)
    for k, (index, weight) in enumerate([(a, 4), (b, 20), (c, 4)]):
        vs.MoveTo(0, 100 * k)
        vs.LineTo(1000, 100 * k)
        h = vs.LNewObj()
        vs.SetLSN(h, -index)
        vs.SetLW(h, weight)
        job.log("  line", k, "lsn", vs.GetLSN(h), "lw", vs.GetLW(h))
    vs.Rect(0, 1000, 1000, 500)
    h = vs.LNewObj()
    vs.SetLSN(h, -a)
    job.log("  rect lsn", vs.GetLSN(h))
    job.log("  unnamed", vs.GetDashStyleIndexN(True, 2, 0.12, 0.18, 0.03, 0.07))


job.step("173_line_types", draw)
