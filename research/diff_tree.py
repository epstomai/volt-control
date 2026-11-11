"""Diff two full tree dumps: every property that changed (meters and bookkeeping ignored)."""
import json, re, sys
a = json.load(open(sys.argv[1], encoding="utf-8")); b = json.load(open(sys.argv[2], encoding="utf-8"))
ign = {"level","peak","clip","change_count","undo_uid","can_undo","can_redo","input_sidechain_uid","touch_count","count_down"}
names = {}
for p, v in b.items():
    pr = (v.get("data") or {}).get("properties") or {}
    if "/buses/" in p and p.count("/") == 4 and "name" in pr: names[p.split("/")[-1]] = pr["name"]["value"]
sub = lambda s: re.sub(r"[0-9a-f]{32}", lambda x: names.get(x.group(), x.group()[:6]), s)
for p in sorted(set(a) | set(b)):
    if p not in a: print("NEW ", sub(p)); continue
    if p not in b: print("GONE", sub(p)); continue
    pa = (a[p].get("data") or {}).get("properties") or {}; pb = (b[p].get("data") or {}).get("properties") or {}
    for k in sorted(set(pa) | set(pb)):
        if k in ign or "/undo_queues/" in p: continue
        if pa.get(k) != pb.get(k):
            print(sub(p), "|", k, "|", json.dumps((pa.get(k) or {}).get("value"), ensure_ascii=False)[:80], "->", json.dumps((pb.get(k) or {}).get("value"), ensure_ascii=False)[:80])
