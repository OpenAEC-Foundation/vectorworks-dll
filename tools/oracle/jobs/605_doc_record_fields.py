# Record formats with one field of each worksheet field type, attached to a rectangle with a
# value set, to match the field kind numbers in the file with the public types; and the user
# origin in feet to see which unit it is stored in.

job.folder("doc")

TYPES = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 14, 15, 16]
VALUES = {1: "42", 2: "TRUE", 3: "3.5", 4: "Hello", 5: "2.25", 6: "1234.5", 7: "0.00123", 8: "1.5", 9: "1500", 10: "45", 11: "6/16/25", 14: "2000000", 15: "3000000000", 16: "25"}


def draw():
    vs.Units(7)
    vs.Rect(0, 100, 100, 0)
    h = vs.LNewObj()
    for t in TYPES:
        vs.NewField("DocFields", "t%02d" % t, VALUES[t], t, 0)
    job.log("fields", vs.NumFields("DocFields") if hasattr(vs, "NumFields") else "?")
    vs.SetRecord(h, "DocFields")
    for t in TYPES:
        vs.SetRField(h, "DocFields", "t%02d" % t, VALUES[t])
    for t in TYPES:
        job.log("t%02d" % t, repr(vs.GetRField(h, "DocFields", "t%02d" % t)))


job.step("605_record_fields", draw)


def feet():
    vs.Units(3)
    vs.SetOriginAbsolute(1, 2)
    vs.Rect(0, 1, 1, 0)
    job.log("origin in feet", vs.GetOrigin(), vs.GetOriginInDocUnits())


job.step("605_origin_feet", feet)
vs.Units(7)
vs.SetOriginAbsolute(0, 0)
