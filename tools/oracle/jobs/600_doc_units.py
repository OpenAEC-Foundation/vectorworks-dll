# Document units: for each standard unit setting save a sample and log the unit preferences
# (public GetPref selectors 150..192 primary, 200..226 secondary), so the document header can
# be matched against known values.

job.folder("doc")

REALS = [150, 152, 176, 180, 186, 187]
STRS = [153, 154, 155, 156, 158, 160, 177, 178, 181, 182]
INTS = [157, 170, 174, 188, 189, 190]
LONGS = [159, 161, 162, 169, 171, 172, 179, 183]
BOOLS = [163, 164, 165, 166, 167, 168, 173, 175, 184, 185, 191]
SEC = {"r": [200, 202], "s": [203, 204, 208, 210], "i": [207, 220, 224], "l": [209, 211, 212, 219, 221, 222], "b": [213, 214, 215, 216, 217, 218, 223, 225]}


def get(fn, sel):
    try:
        return fn(sel)
    except Exception as e:
        return "ERR"


def dump(tag):
    out = []
    for s in REALS + SEC["r"]:
        out.append("%d=%r" % (s, get(vs.GetPrefReal, s)))
    for s in STRS + SEC["s"]:
        out.append("%d=%r" % (s, get(vs.GetPrefString, s)))
    for s in INTS + SEC["i"]:
        out.append("%d=%r" % (s, get(vs.GetPrefInt, s)))
    for s in LONGS + SEC["l"]:
        out.append("%d=%r" % (s, get(vs.GetPrefLongInt, s)))
    for s in BOOLS + SEC["b"]:
        out.append("%d=%r" % (s, get(vs.GetPref, s)))
    job.log(tag, " ".join(out))
    job.log(tag, "GetUnits", vs.GetUnits())


for u in range(1, 11):

    def draw(u=u):
        vs.Units(u)
        vs.Rect(0, 1000, 1000, 0)
        dump("units%d" % u)

    job.step("601_units_%02d" % u, draw)

# restore millimetres
vs.Units(7)
