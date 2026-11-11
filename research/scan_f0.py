"""Find code in UAMixerHelper that stores the immediate byte 0xF0 into memory and then stores a
second small immediate right after it (the Volt HID frame header `f0 <cmd>`). Prints each site with
the command byte so the full host->device command set can be listed. Read-only static scan."""
import collections
import sys

import capstone
import pefile

PATH = r"C:\Program Files (x86)\Universal Audio\UAD Console\UAMixerHelper.exe"
pe = pefile.PE(PATH, fast_load=True)
text = next(s for s in pe.sections if s.Name.rstrip(b"\0") == b".text")
code = text.get_data()
base = pe.OPTIONAL_HEADER.ImageBase + text.VirtualAddress
md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
md.detail = True

hits = collections.defaultdict(list)
i = code.find(b"\xc6")
while i != -1:
    # mov byte ptr [mem], imm8 with imm == 0xF0: opcode C6, ModRM reg=0
    for insn in md.disasm(code[i:i + 12], base + i, 1):
        if insn.mnemonic == "mov" and insn.op_str.startswith("byte ptr") and insn.op_str.endswith("0xf0"):
            mem = insn.op_str.split("[")[1].split("]")[0]
            # look at the next 12 instructions for a store to the following byte
            for nxt in md.disasm(code[i + insn.size:i + insn.size + 80], insn.address + insn.size, 12):
                if nxt.mnemonic == "mov" and nxt.op_str.startswith("byte ptr") and "0x" in nxt.op_str.split(",")[-1]:
                    m2 = nxt.op_str.split("[")[1].split("]")[0]
                    if m2 != mem:
                        imm = nxt.op_str.split(",")[-1].strip()
                        hits[imm].append((insn.address - pe.OPTIONAL_HEADER.ImageBase, mem, m2))
                        break
    i = code.find(b"\xc6", i + 1)

for imm in sorted(hits, key=lambda s: int(s, 16)):
    sites = hits[imm]
    print(f"cmd {imm:>5}: {len(sites):3d} sites  e.g. " + ", ".join(f"+0x{a:x} [{m}]->[{n}]" for a, m, n in sites[:3]))
