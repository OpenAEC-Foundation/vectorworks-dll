# Materials ladder 7: a bitmap on the drawing at a known place. ImportImageFile put the image
# far from the point it was given (job 205), so here the image is moved to its place after the
# import: its top left corner to (100, 200), then (300, 200), then turned by 30 degrees.
# The PNG is the one job 205 wrote (8 x 4 pixels). Millimetres.

job.folder("mat")
PNG = os.path.join(job.folder("mat"), "quarters.png")


def bitmap(at=(100, 200), turn=0):
    def draw():
        vs.ImportImageFile(PNG, at)
        h = vs.FSActLayer()
        (x0, y0), (x1, y1) = vs.GetBBox(h)
        vs.HMove(h, at[0] - x0, at[1] - y0)
        if turn:
            vs.HRotate(h, at, turn)
        job.log("  bitmap type", vs.GetTypeN(h), "box", vs.GetBBox(h), "width", vs.HWidth(h), "height", vs.HHeight(h), "pixels", vs.GetObjectVariableLongInt(h, 534), vs.GetObjectVariableLongInt(h, 535))
    return draw


if os.path.exists(PNG):
    job.step("264_bitmap_at", bitmap())
    job.step("265_bitmap_at_x", bitmap(at=(300, 200)))
    job.step("266_bitmap_turned", bitmap(turn=30))
else:
    job.log("no", PNG)
