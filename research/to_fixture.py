"""Converts a dump_tree.py dump into Volt Control's export format (the test fixture format)."""
import json, sys
src, dst = sys.argv[1], sys.argv[2]
t = json.load(open(src, encoding="utf-8"))
keep = ("/", "/devices", "/auto_gain")
nodes = {p: o.get("data") for p, o in t.items()
         if "timeout" not in o and (p == "/" or any(p == k or p.startswith(k + "/") for k in keep[1:]))}
dev = next(p for p in nodes if p.count("/") == 2 and p.startswith("/devices/"))
out = {"format": "volt-control-tree/1", "app_version": "research", "device_type": nodes[dev]["properties"]["device_type"]["value"], "nodes": nodes}
json.dump(out, open(dst, "w", encoding="utf-8"), ensure_ascii=False, separators=(",", ":"))
print(len(nodes), "nodes ->", dst)
