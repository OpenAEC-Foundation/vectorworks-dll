# Text styles with other font, size and style; scripts with line breaks and other text.
# Millimetres. See 700_kinds_resources.py for the helpers.

job.folder("kinds")


def tryit(label, fn):
    try:
        r = fn()
        job.log("  ", label, "->", r)
        return r
    except Exception as e:
        job.log("  ", label, "FAILED", e)
        return None


def styles():
    fid = vs.GetFontID("Arial Black")
    job.log("font id Arial Black", fid, "Times New Roman", vs.GetFontID("Times New Roman"), "Arial", vs.GetFontID("Arial"))
    h = vs.CreateTextStyleRes("TSfont")
    tryit("font", lambda: vs.SetTextFont(h, 0, 0, fid))
    h = vs.CreateTextStyleRes("TSsize")
    tryit("size", lambda: vs.SetTextSize(h, 0, 0, 24))
    h = vs.CreateTextStyleRes("TSbold")
    tryit("bold", lambda: vs.SetTextStyle(h, 0, 0, 1))
    h = vs.CreateTextStyleRes("TSjust")
    tryit("justify", lambda: vs.SetTextJust(h, 3))
    h = vs.CreateTextStyleRes("TSfontsize")
    tryit("font", lambda: vs.SetTextFont(h, 0, 0, vs.GetFontID("Times New Roman")))
    tryit("size", lambda: vs.SetTextSize(h, 0, 0, 18))
    tryit("style", lambda: vs.SetTextStyle(h, 0, 0, 2))


def scripts():
    tryit("py x", lambda: vs.CreateScriptResource("PyX", "PalC", False, "x=1\r\ny=2", True))
    tryit("vs cr", lambda: vs.CreateScriptResource("VsCr", "PalC", False, "a;\rb;\rc;", False))
    tryit("vs lf", lambda: vs.CreateScriptResource("VsLf", "PalC", False, "a;\nb;", False))
    tryit("vs text", lambda: vs.CreateScriptResource("VsText", "PalC", False, "Message('x');", False))
    tryit("py text", lambda: vs.CreateScriptResource("PyText", "PalC", False, "vs.Message('x')", True))
    tryit("get", lambda: vs.GetScriptResource("PyText"))


job.step("702_kinds_text_styles_scripts", lambda: (styles(), scripts()))
