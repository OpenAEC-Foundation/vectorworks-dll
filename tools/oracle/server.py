# Oracle server: runs inside Vectorworks and makes sample files on request.
#
# Start once: Vectorworks > Extra (Tools) > Plug-ins > Script uitvoeren (Run Script) and choose
# this file. Start it from an empty, unsaved document. Vectorworks then looks frozen ("reageert
# niet"): that is this script waiting for jobs. It ends when a file named `stop` appears in the
# jobs folder, or after eight hours.
#
# A job is a Python file dropped into ROOT\jobs. It runs with `vs` and the helpers below in
# scope; its output goes to <job>.log next to it and the job moves to jobs\done. Jobs run in
# name order.
#
# Everything here uses the public scripting API only (github.com/Vectorworks/developer-scripting).

import os
import time
import traceback

import vs

ROOT = r"C:\Users\joasg\vwx-samples"
JOBS = os.path.join(ROOT, "jobs")
DONE = os.path.join(JOBS, "done")
HOURS = 8


class Job:
    """What a job sees: the output folder, a log, and helpers to make one sample per call."""

    def __init__(self, name):
        self.name = name
        self.lines = []
        self.out = os.path.join(ROOT, "ladder")

    def log(self, *parts):
        self.lines.append(" ".join(str(p) for p in parts))

    def folder(self, *parts):
        """Sets (and makes) the folder samples are saved in, under ROOT."""
        self.out = os.path.join(ROOT, *parts)
        os.makedirs(self.out, exist_ok=True)
        return self.out

    def clear(self):
        """Deletes every object on the active layer."""
        vs.SelectAll()
        vs.DeleteObjs()

    def save(self, name):
        """Saves the document as <out>\\<name>.vwx; an older file of that name is replaced."""
        path = os.path.join(self.out, name + ".vwx")
        if os.path.exists(path):
            os.remove(path)
        code = vs.SaveActiveDocument(path)
        size = os.path.getsize(path) if os.path.exists(path) else -1
        self.log("save", name, "code", code, "bytes", size)
        return path

    def step(self, name, draw, exports=()):
        """One sample: clear, draw, save, and the exports asked for ('ifc', 'stl')."""
        try:
            self.clear()
            draw()
            path = self.save(name)
            base = path[:-4]
            if "ifc" in exports:
                self.log("  ifc", vs.IFC_ExportNoUI(base + ".ifc"))
            if "stl" in exports:
                self.log("  stl", vs.ExportSTL(base + ".stl", False, 50, 1))
        except Exception:
            self.log("FAILED", name)
            self.log(traceback.format_exc())


def run(path):
    name = os.path.basename(path)
    job = Job(name)
    start = time.time()
    try:
        with open(path, encoding="utf-8") as f:
            code = compile(f.read(), path, "exec")
        exec(code, {"vs": vs, "job": job, "os": os, "__name__": "__job__"})
        job.log("job ok")
    except Exception:
        job.log("JOB FAILED")
        job.log(traceback.format_exc())
    job.log("seconds", round(time.time() - start, 1))
    with open(os.path.join(DONE, name + ".log"), "w", encoding="utf-8") as f:
        f.write("\n".join(job.lines) + "\n")
    os.replace(path, os.path.join(DONE, name))


def main():
    os.makedirs(DONE, exist_ok=True)
    stop = os.path.join(JOBS, "stop")
    alive = os.path.join(JOBS, "server.alive")
    deadline = time.time() + HOURS * 3600
    while time.time() < deadline and not os.path.exists(stop):
        with open(alive, "w") as f:
            f.write(str(time.time()))
        for name in sorted(os.listdir(JOBS)):
            if name.endswith(".py"):
                run(os.path.join(JOBS, name))
        time.sleep(0.5)
    for path in (stop, alive):
        if os.path.exists(path):
            os.remove(path)


main()
