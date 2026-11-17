# Stories, level types and layers on stories: one sample with known values, and a log of what
# the scripting interface reports for them.

job.folder("doc")


def layers():
    out = []
    h = vs.FLayer()
    while h:
        out.append(h)
        h = vs.NextLayer(h)
    return out


def tidy():
    ls = layers()
    for h in ls[1:]:
        vs.DelObject(h)
    vs.Units(7)


def dump(tag):
    job.log(tag, "stories", vs.GetNumStories(), "level types", vs.GetNumLayerLevelTypes())
    for i in range(1, vs.GetNumLayerLevelTypes() + 1):
        job.log("  level type", i, repr(vs.GetLevelTypeName(i)))
    for h in layers():
        st = vs.GetStoryOfLayer(h)
        job.log("  layer", repr(vs.GetLName(h)), "type", vs.GetObjectVariableInt(h, 154), "elev", vs.GetLayerElevation(h) if hasattr(vs, "GetLayerElevation") else "?", "levelType", repr(vs.GetLayerLevelType(h)), "story", repr(vs.GetName(st)) if st else None, "scale", vs.GetLScale(h), "wallh", vs.GetWallHeight(h) if hasattr(vs, "GetWallHeight") else "?")
    n = vs.GetNumLevelTemplates()
    job.log("  level templates", n)
    for i in range(n):
        job.log("   ", vs.GetLevelTmpltInfoN(i))


def make():
    tidy()
    dump("start")
    first = layers()[0]
    # story 1
    vs.CreateLayerLevelType("Sample Floor")
    vs.CreateLayerLevelType("Sample Ceiling")
    for n, (name, suf, base) in enumerate([("Ground", "G", 0.0), ("First", "1", 3000.0), ("Second", "2", 6100.0)]):
        ok = vs.CreateStory(name, suf)
        job.log("CreateStory", name, ok)
        sh = vs.GetObject(name)
        lh = vs.CreateLayer("Plan-" + suf, 1)
        vs.SetLayerLevelType(lh, "Sample Floor")
        job.log(" assoc", vs.AssociateLayerWithStory(lh, sh))
        job.log(" elev", vs.SetStoryElevation(sh, base))
        job.log(" add level", vs.AddStoryLevelN(sh, "Sample Ceiling", 2700.0, "Ceiling-" + suf))
    dump("after")
    for i in range(1, vs.GetNumStories() + 1):
        pass


job.step("602_stories", make)
vs.Units(7)
