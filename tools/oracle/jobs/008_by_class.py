# Fifth ladder: which attributes an object takes from its class. Millimetres.

job.folder("ladder", "2027")


def rect_in_class(then=lambda h: None, use=False):
    vs.NameClass("PerKlasse")
    vs.SetClFillBack("PerKlasse", (65535, 0, 0))
    vs.SetClPenFore("PerKlasse", (0, 0, 65535))
    vs.SetClLW("PerKlasse", 40)
    vs.SetClUseGraphic("PerKlasse", use)
    vs.Rect(0, 500, 1000, 0)
    h = vs.LNewObj()
    then(h)
    job.log("  class", vs.GetClass(h), "fill by class", vs.IsFillColorByClass(h), "pen by class", vs.IsPenColorByClass(h), "pattern by class", vs.IsFPatByClass(h), "weight by class", vs.IsLWByClass(h))
    vs.NameClass("Geen")
    return h


job.step("230_class_own", lambda: rect_in_class())
job.step("231_class_fill_colour", lambda: rect_in_class(vs.SetFillColorByClass))
job.step("232_class_pen_colour", lambda: rect_in_class(vs.SetPenColorByClass))
job.step("233_class_fill_pattern", lambda: rect_in_class(vs.SetFPatByClass))
job.step("234_class_line_weight", lambda: rect_in_class(vs.SetLWByClass))
job.step("235_class_all", lambda: rect_in_class(use=True))
