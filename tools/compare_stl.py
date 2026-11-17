# Compares the bounds of the reader's meshes (metres) with those of Vectorworks' STL export
# (millimetres) of the same sample:  python compare_stl.py <vwx.exe> <folder with .vwx and .stl>
import sys, subprocess, json, glob, os, re
exe, folder = sys.argv[1], sys.argv[2]
ok = bad = none = 0
for stl in sorted(glob.glob(os.path.join(folder, '*.stl'))):
    vwx = stl[:-4] + '.vwx'
    pts = [tuple(map(float, m.groups())) for m in re.finditer(r'vertex\s+(\S+)\s+(\S+)\s+(\S+)', open(stl, errors='replace').read())]
    if not pts: continue
    lo = [min(p[k] for p in pts) / 1000 for k in range(3)]; hi = [max(p[k] for p in pts) / 1000 for k in range(3)]
    r = json.loads(subprocess.run([exe, 'mesh', vwx], capture_output=True, text=True).stdout or '{}')
    name = os.path.basename(vwx)
    if not r.get('triangles'):
        none += 1; print(f'  none  {name:<28} stl {[round(v, 3) for v in lo]} .. {[round(v, 3) for v in hi]} ({len(pts)//3} tris)'); continue
    # the STL export is relative to the layer; the reader gives heights in the document
    shift = r['min'][2] - lo[2]
    lo[2] += shift; hi[2] += shift
    d = max(max(abs(a - b) for a, b in zip(lo, r['min'])), max(abs(a - b) for a, b in zip(hi, r['max'])))
    if d < 2e-3: ok += 1; print(f'  ok    {name:<28} {r["triangles"]} tris (stl {len(pts)//3})' + (f', on a layer at {shift:.3f} m' if abs(shift) > 1e-6 else ''))
    else: bad += 1; print(f'  DIFF  {name:<28} ours {[round(v, 3) for v in r["min"]]} .. {[round(v, 3) for v in r["max"]]}  stl {[round(v, 3) for v in lo]} .. {[round(v, 3) for v in hi]}')
print(f'{ok} match, {bad} differ, {none} not drawn yet')
