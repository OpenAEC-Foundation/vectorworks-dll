# Loci (2D and 3D) at known points, and a roof object (type 83) built from four edges, to read
# where a locus keeps its point and what a roof holds. Millimetres, on the first layer.

job.folder("cov")
vs.Layer("Ontwerplaag-1")
X = ("stl",)

job.step("401_locus2d", lambda: vs.Locus((100, 200)))
job.step("402_locus3d", lambda: vs.Locus3D((1000, 2000, 3000)))


def two_loci():
    vs.Locus3D((1000, 2000, 3000))
    vs.Locus3D((-500, 250, -75))


job.step("403_two_loci3d", two_loci)


def roof():
    h = vs.CreateRoof(False, 0, 200, 1, 0)
    # a 6 m by 4 m plan, counter-clockwise, 30 degrees, 500 overhang, eaves at 2700
    for p in [(0, 0), (6000, 0), (6000, 4000), (0, 4000)]:
        vs.AppendRoofEdge(h, p, 30, 500, 2700)
    vs.ResetObject(h)
    job.log("roof type", vs.GetTypeN(h), "edges", [vs.GetRoofEdge(h, k) for k in range(1, 5)])
    job.log("elements", vs.GetNumRoofElements(h))


job.step("404_roof", roof, X)
