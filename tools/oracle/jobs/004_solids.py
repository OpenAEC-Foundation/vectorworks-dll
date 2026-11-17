# Third ladder: sweeps (partial, with pitch, of a circle), CSG solids with curved faces, a moved
# CSG solid, rounded profiles, a multiple extrude of three profiles. Millimetres, on the first
# layer. See 001_ladder.py for the helpers.

job.folder("ladder", "2027")
# the second layer still holds an object of an earlier job: job.clear() empties the active layer
vs.Layer("Verdieping 1")
job.clear()
vs.Layer("Ontwerplaag-1")
vs.NameClass("Geen")
X = ("stl",)


def extrude(z0, z1, profile=lambda: vs.Rect(0, 500, 1000, 0)):
    vs.BeginXtrd(z0, z1)
    profile()
    vs.EndXtrd()
    return vs.LNewObj()


def sweep(start, arc, step, pitch, profile=lambda: vs.Rect(500, 300, 1000, 0)):
    vs.BeginSweep(start, arc, step, pitch)
    profile()
    vs.EndSweep()
    return vs.LNewObj()


def cylinder(x, y, r, z0, z1):
    return extrude(z0, z1, lambda: vs.ArcByCenter(x, y, r, 0, 360))


# --- sweeps ------------------------------------------------------------------------------
job.step("200_sweep_partial", lambda: sweep(30, 200, 10, 0), X)
job.step("201_sweep_pitch", lambda: sweep(0, 720, 15, 400), X)
job.step("202_sweep_circle", lambda: sweep(0, 360, 15, 0, lambda: vs.ArcByCenter(800, 0, 100, 0, 360)), X)
job.step("203_sweep_quarter", lambda: sweep(0, 90, 15, 0), X)

# --- CSG solids with curved faces ----------------------------------------------------------
job.step("210_box_minus_cylinder", lambda: vs.SubtractSolid(extrude(0, 1000), cylinder(500, 250, 150, -100, 1100)), X)
job.step("211_box_plus_cylinder", lambda: vs.AddSolid(extrude(0, 1000), cylinder(500, 250, 150, 500, 1500)), X)


def box_and_sphere():
    box = extrude(0, 1000)
    vs.CreateSphere((500, 250, 1000), 300)
    return box, vs.LNewObj()


def box_and_cone():
    box = extrude(0, 1000)
    vs.CreateCone((500, 250, 1000), (500, 250, 1600), 200)
    return box, vs.LNewObj()


job.step("212_box_plus_sphere", lambda: vs.AddSolid(*box_and_sphere()), X)
job.step("213_box_minus_sphere", lambda: vs.SubtractSolid(*box_and_sphere()), X)
job.step("214_box_plus_cone", lambda: vs.AddSolid(*box_and_cone()), X)


def moved():
    vs.AddSolid(extrude(0, 1000), cylinder(500, 250, 150, 500, 1500))
    h = vs.LNewObj()
    vs.Move3DObj(h, 2000, 1000, 500)
    job.log("  moved solid type", vs.GetTypeN(h))


def turned():
    vs.AddSolid(extrude(0, 1000), cylinder(500, 250, 150, 500, 1500))
    h = vs.LNewObj()
    vs.Set3DRot(h, 0, 0, 90, 0, 0, 0)
    vs.Move3DObj(h, 2000, 0, 0)


job.step("215_solid_moved", moved, X)
job.step("216_solid_turned", turned, X)

# --- rounded profiles ----------------------------------------------------------------------
job.step("220_extrude_rounded_rect", lambda: extrude(0, 1000, lambda: vs.RRect(0, 500, 1000, 0, 200, 200)), X)
job.step("221_extrude_oval", lambda: extrude(0, 1000, lambda: vs.Oval(0, 500, 1000, 0)), X)


def three_profiles():
    vs.BeginMXtrd(0, 3000)
    vs.Rect(0, 1000, 1000, 0)
    vs.ArcByCenter(500, 500, 300, 0, 360)
    vs.Rect(400, 600, 600, 400)
    vs.EndMXtrd()


job.step("222_multiple_extrude_three", three_profiles, X)
