# Materials ladder 6: lights moved and switched with the general object calls (the light calls
# alone did not change the file for height, on/off and shadows). Millimetres.

job.folder("mat")


def describe(h):
    job.log("  light info", vs.GetLightInfo(h), "at", vs.GetLightLocation(h), "on", vs.GetObjectVariableBoolean(h, 50))


def light(then):
    def draw():
        h = vs.CreateLight(1000, 2000, 0, 1, True, False)
        then(h)
        vs.ResetObject(h)
        describe(h)
    return draw


job.step("270_light_base", light(lambda h: None))
job.step("271_light_moved_up", light(lambda h: vs.Move3DObj(h, 0, 0, 500)))
job.step("272_light_moved_x", light(lambda h: vs.Move3DObj(h, 700, 0, 0)))
job.step("273_light_off_variable", light(lambda h: vs.SetObjectVariableBoolean(h, 50, False)))
