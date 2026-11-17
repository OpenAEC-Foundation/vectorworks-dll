# User origin, georeference, project elevation and drawing size, one change per sample, each
# with what the scripting interface reports.

job.folder("doc")


def report(tag):
    try:
        job.log(tag, "origin", vs.GetOrigin(), "inDocUnits", vs.GetOriginInDocUnits(), "projElev", vs.GetProjectElevation(), "angleToNorth", vs.GetAngleToNorth())
    except Exception as e:
        job.log(tag, "ERR", e)
    try:
        job.log(tag, "georef", vs.IsGeoreferenced(None), "gis", vs.GetGISOriginN())
    except Exception as e:
        job.log(tag, "georef ERR", e)
    try:
        job.log(tag, "wkt", vs.GetProjectionWKT(vs.ActLayer(), False))
    except Exception as e:
        job.log(tag, "wkt ERR", e)
    try:
        job.log(tag, "drawing rect", vs.GetDrawingSizeRect())
    except Exception as e:
        job.log(tag, "rect ERR", e)
    for sel in (6702, 80, 81, 70, 78, 79):
        try:
            job.log(tag, "pref", sel, vs.GetPrefReal(sel))
        except Exception as e:
            job.log(tag, "pref ERR", sel)


def step(name, f):
    def draw():
        f()
        vs.Rect(0, 100, 100, 0)
        report(name)

    job.step(name, draw)


vs.Units(7)
step("604_base", lambda: vs.SetOriginAbsolute(0, 0))
step("604_origin", lambda: vs.SetOriginAbsolute(1000, 2000))
step("604_origin2", lambda: vs.SetOriginAbsolute(-3000, 500))
step("604_elev", lambda: vs.SetProjectElevation(1234))
step("604_georef_rd", lambda: (vs.SetOriginAbsolute(1000, 2000), vs.SetDocGeoRefByUsrOrg(28992)))
step("604_georef_utm", lambda: (vs.SetOriginAbsolute(5000, -4000), vs.SetDocGeoRefByUsrOrg(32631)))
step("604_nogeoref", lambda: vs.RemoveGeoref(None))
step("604_drwsize", lambda: vs.DrwSize(3, 4))
step("604_drwsize2", lambda: vs.DrwSize(1, 1))
vs.SetOriginAbsolute(0, 0)
