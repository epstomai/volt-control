import os, struct, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from disfn import img, md
for name, tbl, n in [("device(cat3)", 0x190b220, 20), ("cat2", 0x190b270, 9)]:
    print("==", name)
    for k in range(n):
        tgt = struct.unpack_from("<I", img, tbl + 4 * k)[0]
        cmd = None; ln = None
        for x in md.disasm(img[tgt:tgt + 200], tgt):
            if x.mnemonic == "mov" and x.op_str.endswith("f0") and "word ptr" in x.op_str or (x.mnemonic=="mov" and x.op_str.split(",")[-1].strip().endswith("f0") and "ptr" in x.op_str):
                cmd = x.op_str.split(",")[-1].strip()
            if x.mnemonic == "mov" and x.op_str.startswith("r8d, 0x") or x.op_str.startswith("r8d, ") and x.op_str[5:].strip().isdigit():
                ln = x.op_str.split(",")[-1].strip()
            if x.mnemonic in ("call", "jmp") and cmd:
                break
            if x.mnemonic == "jmp" and not cmd:
                cmd = f"(jmp {x.op_str})"; break
        print(f"  case {k:2d} -> 0x{tgt:x}  cmd={cmd} len={ln}")
