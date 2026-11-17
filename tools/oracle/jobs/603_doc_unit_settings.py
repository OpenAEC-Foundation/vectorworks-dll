# One unit setting changed at a time from millimetres, to find where each lives in the document
# header.

job.folder("doc")


def one(name, setter):
    def draw():
        vs.Units(7)
        setter()
        vs.Rect(0, 100, 100, 0)
        job.log(name, "units", vs.GetUnits(), "area", vs.GetPrefString(177), vs.GetPrefString(178), vs.GetPrefReal(176), vs.GetPrefLongInt(179), "vol", vs.GetPrefString(181), vs.GetPrefString(182), vs.GetPrefReal(180), vs.GetPrefLongInt(183), "dimprec", vs.GetPrefLongInt(169), "frac", vs.GetPrefLongInt(171), vs.GetPrefLongInt(172))

    job.step(name, draw)


one("603_base", lambda: None)
one("603_dimprec5", lambda: vs.SetPrefLongInt(169, 5))
one("603_dispfrac4", lambda: vs.SetPrefLongInt(162, 4))
one("603_areaprec5", lambda: vs.SetPrefLongInt(179, 5))
one("603_volprec6", lambda: vs.SetPrefLongInt(183, 6))
one("603_fracdisp7", lambda: vs.SetPrefLongInt(171, 7))
one("603_fracdim8", lambda: vs.SetPrefLongInt(172, 8))
one("603_showmark", lambda: vs.SetPref(163, True))
one("603_trailing", lambda: vs.SetPref(165, True))
one("603_leadzero_off", lambda: vs.SetPref(164, False))
one("603_thousands", lambda: vs.SetPref(191, True))
one("603_areaname", lambda: (vs.SetPrefString(177, "Hectares"), vs.SetPrefString(178, " ha"), vs.SetPrefReal(176, 15.5), vs.SetPref(184, True)))
one("603_volname", lambda: (vs.SetPrefString(181, "Liters"), vs.SetPrefString(182, " L"), vs.SetPrefReal(180, 0.0163871), vs.SetPref(185, True)))
one("603_anglerad", lambda: vs.SetPrefInt(174, 1))
one("603_angleprec", lambda: vs.SetPrefInt(120, 4))
one("603_rounding", lambda: vs.SetPrefInt(188, 2))
vs.Units(7)
