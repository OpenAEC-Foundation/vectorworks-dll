# The colour palette of the document as the scripting API sees it: index -> RGB (16 bit).
job.folder("ladder", "2027")
lines = []
for i in range(0, 2000):
    try:
        r, g, b = vs.ColorIndexToRGB(i)
        lines.append("%d %d %d %d" % (i, r, g, b))
    except Exception as e:
        lines.append("%d error %s" % (i, e))
with open(os.path.join(job.out, "palette.txt"), "w") as f:
    f.write("\n".join(lines) + "\n")
job.log("palette", len(lines))
