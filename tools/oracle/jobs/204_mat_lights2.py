# Materials ladder 4: where a light keeps its place, on/off and shadows (CreateLight alone did
# not change the file for those). Millimetres.

job.folder("mat")


def describe(h):
    job.log("  light info", vs.GetLightInfo(h), "at", vs.GetLightLocation(h), "box", vs.GetBBox(h))


def light(at=None, info=None, kind=1):
    def draw():
        h = vs.CreateLight(0, 0, 0, kind, True, False)
        if at:
            vs.SetLightLocation(h, (at[0], at[1]), at[2])
        if info:
            vs.SetLightInfo(h, *info)
        vs.ResetObject(h)
        describe(h)
    return draw


job.step("250_light_at", light(at=(1000, 2000, 3000)))
job.step("251_light_at_z", light(at=(1000, 2000, 3500)))
job.step("252_light_at_x", light(at=(1500, 2000, 3000)))
job.step("253_light_off", light(at=(1000, 2000, 3000), info=(1, 100, False, False)))
job.step("254_light_shadow", light(at=(1000, 2000, 3000), info=(1, 100, True, True)))
job.step("255_spot_at", light(at=(1000, 2000, 3000), kind=2))
