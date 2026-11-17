# Materials ladder 2: lights. Each sample changes one thing of the light before it, so that a
# pair of files shows where that thing is kept. Millimetres; colours 0..65535.

job.folder("mat")


def describe(h):
    job.log("  light type", vs.GetTypeN(h), "info", vs.GetLightInfo(h), "at", vs.GetLightLocation(h), "colour", vs.GetLightColorRGB(h))
    job.log("   direction", vs.GetLightDirection(h), "falloff", vs.GetLightFalloff(h), "beam", vs.GetBeamAngle(h), "spread", vs.GetSpreadAngle(h))


def light(kind=1, at=(1000, 2000, 3000), on=True, shadow=False, then=lambda h: None):
    def draw():
        h = vs.CreateLight(at[0], at[1], at[2], kind, on, shadow)
        then(h)
        describe(h)
    return draw


def seq(*fs):
    def run(h):
        for f in fs:
            f(h)
    return run


job.step("220_light_point", light())
job.step("221_light_point_z", light(at=(1000, 2000, 3500)))
job.step("222_light_point_red", light(then=lambda h: vs.SetLightColorRGB(h, 65535, 0, 0)))
job.step("223_light_point_dim", light(then=lambda h: vs.SetLightInfo(h, 1, 40, True, False)))
job.step("224_light_point_off", light(on=False))
job.step("225_light_point_shadow", light(shadow=True))
job.step("226_light_point_falloff", light(then=lambda h: vs.SetLightFalloff(h, 2, 0)))
job.step("227_light_directional", light(kind=0, then=lambda h: vs.SetLightDirection(h, 30, 45)))
job.step("228_light_directional_pan", light(kind=0, then=lambda h: vs.SetLightDirection(h, 60, 45)))
job.step("229_light_directional_tilt", light(kind=0, then=lambda h: vs.SetLightDirection(h, 60, 20)))
job.step("230_light_spot", light(kind=2, then=lambda h: vs.SetLightDirection(h, 30, 45)))
job.step("231_light_spot_beam", light(kind=2, then=seq(lambda h: vs.SetLightDirection(h, 30, 45), lambda h: vs.SetBeamAngle(h, 20), lambda h: vs.SetSpreadAngle(h, 50))))
job.step("232_light_spot_tilt", light(kind=2, then=lambda h: vs.SetLightDirection(h, 30, -60)))
