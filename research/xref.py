"""Find code that references given ASCII strings (RIP-relative lea/mov). Read-only.
usage: python -I xref.py <string> [...]"""
import re, struct, sys
sys.path.insert(0, __file__.rsplit("\\", 1)[0] if "\\" in __file__ else ".")
from disfn import pe, img, func_of

text = next(s for s in pe.sections if s.Name.startswith(b".text"))
tb, te = text.VirtualAddress, text.VirtualAddress + text.Misc_VirtualSize
code = img[tb:te]
for want in sys.argv[1:]:
    for m in re.finditer(re.escape(want.encode()) + b"\x00", img):
        srva = m.start()
        if img[srva - 1] != 0:
            continue  # inside a longer string
        hits = []
        for i in range(len(code) - 7):
            # REX 48/4C + 8D (lea) / 8B (mov) + modrm with rip-relative (mod=00, rm=101)
            if code[i] in (0x48, 0x4C) and code[i + 1] in (0x8D, 0x8B) and (code[i + 2] & 0xC7) == 0x05:
                disp = struct.unpack_from("<i", code, i + 3)[0]
                if tb + i + 7 + disp == srva:
                    hits.append(tb + i)
        print(f"{want!r} @ rva 0x{srva:x}: " + ", ".join(f"0x{h:x} (func 0x{func_of(h)[0]:x})" for h in hits))
