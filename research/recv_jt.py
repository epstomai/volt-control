import os, re, struct, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from disfn import img, md
default = 0x190c2a3
tgts = {}
for k in range(0x23):
    t = struct.unpack_from("<I", img, 0x190c4e0 + 4 * k)[0]
    if t != default:
        tgts.setdefault(t, []).append(k + 1)
def cstr(rva):
    end = img.find(b"\0", rva)
    return img[rva:end].decode("latin1")
for t, cmds in sorted(tgts.items()):
    strs = []
    n = 0
    for x in md.disasm(img[t:t + 600], t):
        n += 1
        m = re.match(r"r\w+, \[rip \+ 0x([0-9a-f]+)\]", x.op_str)
        if x.mnemonic == "lea" and m:
            a = x.address + x.size + int(m.group(1), 16)
            if 0x2424000 <= a < 0x2e50000:
                s = cstr(a)
                if len(s) > 3 and s.isprintable():
                    strs.append(s[:90])
        if x.mnemonic == "jmp" and n > 3 or n > 120:
            break
    print(f"cmd {','.join(hex(c) for c in cmds):10s} -> 0x{t:x}  {strs[:3]}")
print("fmt:", cstr(0x190b320 + 0xc3c290))
