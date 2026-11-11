"""Find direct call/jmp (E8/E9 rel32) xrefs to given RVAs, and RIP-relative refs (any insn bytes pattern disp32) to data RVAs. Read-only.
usage: python -I callx.py [-d] <rva-hex> [...]   (-d: data refs via rip-relative disp scan)"""
import struct, sys
sys.path.insert(0, __file__.rsplit("\\", 1)[0] if "\\" in __file__ else ".")
from disfn import pe, img, func_of
import numpy as np
text = next(s for s in pe.sections if s.Name.startswith(b".text"))
tb, te = text.VirtualAddress, text.VirtualAddress + text.Misc_VirtualSize
code = np.frombuffer(bytes(img[tb:te]), dtype=np.uint8)
n = len(code)
args = sys.argv[1:]
data = False
if args and args[0] == "-d":
    data = True; args = args[1:]
# int32 at each offset
d32 = (code[:n-3].astype(np.int64) | (code[1:n-2].astype(np.int64) << 8) | (code[2:n-1].astype(np.int64) << 16) | (code[3:n].astype(np.int64) << 24))
d32 = np.where(d32 >= 2**31, d32 - 2**32, d32)
pos = np.arange(n-3, dtype=np.int64) + tb
for a in args:
    t = int(a, 16)
    out = []
    if not data:
        # disp at i+1, next insn at i+5
        cand = np.nonzero(pos[:-1] + 5 + d32[1:] == t)[0]
        for c in cand:
            if code[c] in (0xE8, 0xE9):
                out.append(tb + c)
    else:
        # disp at j, next insn ends at j+4+k where k = imm bytes (0,1,4)
        for k in (0, 1, 2, 4):
            cand = np.nonzero(pos + 4 + k + d32 == t)[0]
            for c in cand:
                out.append((tb + c, k))
    if data:
        print(f"data 0x{t:x}: " + ", ".join(f"disp@0x{h:x}(+{k}) f0x{func_of(h)[0]:x}" for h, k in sorted(set(out))))
    else:
        print(f"call 0x{t:x}: " + ", ".join(f"0x{h:x}(f0x{func_of(h)[0]:x})" for h in out))
