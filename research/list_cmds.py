"""List every call to the Volt HID send function (0x190c680) and the frame header/len built before it."""
import re
import struct

import sys, os; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from disfn import funcs, img, md

SEND = 0x190c680
text_lo, text_hi = 0x1000, 0x1000 + 0x237b76b
# find E8 rel32 calls targeting SEND
sites = []
mv = memoryview(img)
i = img.find(b"\xe8", text_lo, text_hi)
while i != -1:
    rel = struct.unpack_from("<i", img, i + 1)[0]
    if i + 5 + rel == SEND:
        sites.append(i)
    i = img.find(b"\xe8", i + 1, text_hi)
print(len(sites), "call sites")
import bisect
starts = [b for b, _ in funcs]
for s in sites:
    fb = funcs[bisect.bisect_right(starts, s) - 1][0]
    lo = max(fb, s - 90)
    # disassemble from function start to keep instruction sync, keep last 14 insns before call
    insns = [x for x in md.disasm(img[fb:s + 5], fb)]
    tail = [x for x in insns if x.address >= lo]
    words = [x.op_str for x in tail if x.mnemonic == "mov" and ("0x" in x.op_str.split(",")[-1]) and ("word ptr" in x.op_str or "byte ptr" in x.op_str or "dword ptr" in x.op_str or "r8d" in x.op_str)]
    print(f"call @0x{s:x} (func 0x{fb:x}): " + " | ".join(words[-6:]))
