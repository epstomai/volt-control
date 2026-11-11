"""Disassemble the whole function containing each given RVA (bounds from .pdata). Read-only.
usage: python -I disfn.py <rva-hex> [...]"""
import bisect
import struct
import sys

import capstone
import pefile

PATH = r"C:\Program Files (x86)\Universal Audio\UAD Console\UAMixerHelper.exe"
pe = pefile.PE(PATH, fast_load=True)
_pd = next(x for x in pe.sections if x.Name.startswith(b".pdata"))
_raw = _pd.get_data()[: _pd.Misc_VirtualSize]
funcs = sorted({struct.unpack_from("<II", _raw, o) for o in range(0, len(_raw) - 11, 12)} - {(0, 0)})
starts = [b for b, _ in funcs]
img = pe.get_memory_mapped_image()
md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)


def func_of(rva):
    i = bisect.bisect_right(starts, rva) - 1
    return funcs[i]


if __name__ == "__main__":
    for arg in sys.argv[1:]:
        rva = int(arg, 16)
        b, e = func_of(rva)
        print(f"=== rva 0x{rva:x} in func 0x{b:x}-0x{e:x} ({e-b} bytes)")
        for insn in md.disasm(img[b:e], b):
            mark = " <==" if insn.address + insn.size == rva else ""
            print(f"  {insn.address:8x}: {insn.mnemonic:6s} {insn.op_str}{mark}")
