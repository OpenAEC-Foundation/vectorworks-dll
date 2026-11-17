# Primitive solids away from the origin and tilted, to see which coordinates their cached mesh
# (type 85) is in. Millimetres, on the first layer.

job.folder("ladder", "2027")
vs.Layer("Ontwerplaag-1")
X = ("stl",)

job.step("300_sphere_moved", lambda: vs.CreateSphere((1000, 2000, 3000), 400), X)
job.step("301_cone_tilted", lambda: vs.CreateCone((1000, 2000, 0), (1600, 2000, 800), 300), X)
job.step("302_hemisphere_sideways", lambda: vs.CreateHemisphere((1000, 0, 500), (1400, 0, 500)), X)


def two():
    vs.CreateSphere((0, 0, 0), 200)
    vs.CreateCone((1000, 0, 0), (1000, 0, 600), 250)


job.step("303_sphere_and_cone", two, X)
