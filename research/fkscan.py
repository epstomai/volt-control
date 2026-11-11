"""Scan all UAFeatureKey static initializers (calls to ctor 0x1a761b4): name, hashed?, flags(r9d), vtable(type), default. Read-only.
usage: python -I fkscan.py [filter-substring]"""
import re, struct, sys
sys.path.insert(0, __file__.rsplit("\\", 1)[0] if "\\" in __file__ else ".")
from disfn import img, func_of, md, pe
from disa import s_at
base = pe.OPTIONAL_HEADER.ImageBase
CTOR = 0x1a761b4
VT = {0x2564d90: 'Bool?'}
names = {}
for n in ('UAFeatureKey_Bool', 'UAFeatureKey_Int', 'UAFeatureKey_Double', 'UAFeatureKey_String', 'UAFeatureKey_License'):
    pass
flt = sys.argv[1] if len(sys.argv) > 1 else None
# collect caller funcs
text = next(s for s in pe.sections if s.Name.startswith(b".text"))
tb = text.VirtualAddress
code = bytes(img[tb:tb + text.Misc_VirtualSize])
callers = set()
for m in re.finditer(b'\xe8', code):
    i = m.start()
    if i + 5 > len(code): break
    if tb + i + 5 + struct.unpack_from('<i', code, i + 1)[0] == CTOR:
        callers.add(func_of(tb + i)[0])
rows = []
for f in sorted(callers):
    b, e = func_of(f)
    ins = list(md.disasm(bytes(img[b:e]), b))
    strs = []; r9 = None; hashed = False; obj = None; vt = None; seen = False; dflt = None
    for k, i in enumerate(ins):
        m = re.search(r'rip ([+-]) 0x([0-9a-f]+)', i.op_str)
        t = None
        if m:
            t = i.address + i.size + int(m.group(2), 16) * (1 if m.group(1) == '+' else -1)
        if i.mnemonic == 'movabs' and '0xcbf29ce484222325' in i.op_str:
            hashed = True
        if not seen:
            if t is not None and i.mnemonic == 'lea':
                s = s_at(t)
                if s: strs.append(s)
                if i.op_str.startswith('rcx'): obj = t
            if i.op_str.startswith('r9d'):
                if i.mnemonic == 'xor': r9 = 0
                elif i.mnemonic == 'mov':
                    try: r9 = int(i.op_str.split(',')[1], 0)
                    except: r9 = i.op_str
            if i.mnemonic == 'call' and i.op_str == hex(CTOR):
                seen = True
        else:
            if vt is None and i.mnemonic == 'lea' and t is not None and i.op_str.startswith('rax'):
                vt = t
    rows.append((f, strs, hashed, r9, obj, vt))
for f, strs, hashed, r9, obj, vt in rows:
    line = f"f=0x{f:x} obj=0x{(obj or 0):x} vt=0x{(vt or 0):x} flags={r9} hashed={hashed} strs={strs}"
    if flt is None or flt in line:
        print(line)
