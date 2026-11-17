# Resources the document keeps: document scripts and script palettes, nested symbol folders, text
# styles, class and layer descriptions. One sample with all of them, then small variations.
# Millimetres. See 001_ladder.py for the helpers.

job.folder("kinds")


def tryit(label, fn):
    try:
        r = fn()
        job.log("  ", label, "->", r)
        return r
    except Exception as e:
        job.log("  ", label, "FAILED", e)
        return None


def scripts():
    tryit("script vs", lambda: vs.CreateScriptResource("ScrVS", "PalA", False, "PROCEDURE Hi;\nBEGIN\n Message('vs');\nEND;\nRun(Hi);", False))
    tryit("script py", lambda: vs.CreateScriptResource("ScrPy", "PalA", False, "import vs\nvs.Message('py')", True))
    tryit("script other palette", lambda: vs.CreateScriptResource("ScrB", "PalB", False, "Message('b');", False))


def folders():
    vs.NameObject("FolderOuter")
    vs.BeginFolderN(16)
    vs.NameObject("FolderInner")
    vs.BeginFolderN(16)
    vs.BeginSym("SymInInner")
    vs.Rect(0, 100, 100, 0)
    vs.EndSym()
    vs.EndFolder()
    vs.BeginSym("SymInOuter")
    vs.Rect(0, 100, 100, 0)
    vs.EndSym()
    vs.EndFolder()
    vs.BeginSym("SymLoose")
    vs.Rect(0, 100, 100, 0)
    vs.EndSym()


def text_styles():
    for name in ("TSone", "TStwo"):
        h = tryit("text style " + name, lambda: vs.CreateTextStyleRes(name))
        job.log("   handle", h)


def descriptions():
    c = vs.GetObject("Descl")
    if c in (None, 0):
        vs.NameClass("Descl")
        c = vs.GetObject("Descl")
    tryit("class description", lambda: vs.SetDescriptionText(c, "Class description text"))
    layer = vs.ActLayer()
    tryit("layer description", lambda: vs.SetDescriptionText(layer, "Layer description text"))


def everything():
    scripts()
    folders()
    text_styles()
    descriptions()


job.step("700_kinds_resources", everything)
job.step("701_kinds_empty", lambda: None)
