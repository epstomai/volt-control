"""Annotated disassembly of whole function(s): resolves RIP-relative targets and shows ASCII/UTF-16 strings. Read-only.
usage: python -I disa.py <rva-hex> [...]"""
import re, sys
sys.path.insert(0, __file__.rsplit("\\", 1)[0] if "\\" in __file__ else ".")
from disfn import img, func_of, md

def s_at(r):
    if r <= 0 or r + 4 > len(img):
        return None
    b = bytes(img[r:r + 160])
    m = re.match(rb'[\x20-\x7e\t\n]{3,}', b)
    if m and len(m.group()) < len(b) and b[len(m.group())] == 0:
        return repr(m.group().decode())
    m = re.match(rb'(?:[\x20-\x7e]\x00){3,}', b)
    if m:
        return 'L' + repr(m.group().decode('utf-16le'))
    return None

for arg in sys.argv[1:]:
    rva = int(arg, 16)
    b, e = func_of(rva)
    print(f"=== func 0x{b:x}-0x{e:x}")
    for i in md.disasm(bytes(img[b:e]), b):
        ann = ""
        m = re.search(r'rip ([+-]) 0x([0-9a-f]+)', i.op_str)
        if m:
            t = i.address + i.size + (int(m.group(2), 16) * (1 if m.group(1) == '+' else -1))
            s = s_at(t)
            ann = f"  ; 0x{t:x}" + (f" {s}" if s else "")
        print(f"  {i.address:8x}: {i.mnemonic:6s} {i.op_str}{ann}")
