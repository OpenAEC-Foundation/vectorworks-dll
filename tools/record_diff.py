# record-level diff of two unpacked bodies: aligns records by (type, length) and prints changed
# byte ranges per record, ignoring the UUID at 0x38
import sys, struct, difflib
def recs(d):
    at = 0; out = []
    while at + 8 <= len(d):
        ln = struct.unpack_from('<Q', d, at)[0]; out.append((at, d[at:at+ln])); at += ln
    return out
def tag(r):
    t = r[0x19]
    return f't{t}' + (':' + r[0x50:0x54][::-1].decode('latin1') if t == 76 else '')
a = recs(open(sys.argv[1], 'rb').read()); b = recs(open(sys.argv[2], 'rb').read())
ka = [(r[0x19], len(r)) for _, r in a]; kb = [(r[0x19], len(r)) for _, r in b]
sm = difflib.SequenceMatcher(None, ka, kb, autojunk=False)
for op, i1, i2, j1, j2 in sm.get_opcodes():
    if op == 'equal':
        for i, j in zip(range(i1, i2), range(j1, j2)):
            ra, rb = a[i][1], b[j][1]
            ch = [k for k in range(len(ra)) if ra[k] != rb[k] and not 0x38 <= k < 0x48]
            if not ch: continue
            runs = []; s = p = ch[0]
            for k in ch[1:]:
                if k > p + 2: runs.append((s, p)); s = k
                p = k
            runs.append((s, p))
            print(f'#{i} {tag(ra)} len {len(ra)} at 0x{a[i][0]:x}:')
            for s, e in runs[:12]:
                e2 = e + 1
                extra = ''
                if e2 - s <= 8 and s + 8 <= len(ra):
                    s8 = s - (s % 2)
                extra = ''
                print(f'    +0x{s:x}..0x{e2:x}  {ra[s:e2].hex(" ")[:72]}  ->  {rb[s:e2].hex(" ")[:72]}')
    else:
        for i in range(i1, i2): print(f'- #{i} {tag(a[i][1])} len {len(a[i][1])} at 0x{a[i][0]:x}')
        for j in range(j1, j2): print(f'+ #{j} {tag(b[j][1])} len {len(b[j][1])} at 0x{b[j][0]:x}')
