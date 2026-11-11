"""Extract an Electron app.asar (read-only on source). usage: python -I asar_x.py <app.asar> <outdir>"""
import json, os, struct, sys
src, out = sys.argv[1], sys.argv[2]
f = open(src, "rb")
_, hsize, _, jlen = struct.unpack("<4I", f.read(16))
header = json.loads(f.read(jlen))
base = 8 + hsize
def walk(node, path):
    for name, ent in node.get("files", {}).items():
        p = os.path.join(path, name)
        if "files" in ent:
            walk(ent, p)
        elif not ent.get("unpacked") and "offset" in ent:
            os.makedirs(os.path.dirname(p), exist_ok=True)
            f.seek(base + int(ent["offset"]))
            open(p, "wb").write(f.read(ent["size"]))
walk(header, out)
print("ok")
