"""Robust watcher: subscribes to all Volt controls; logs pushes, errors, disconnects, heartbeats."""
import socket, json, time, sys, re
DEV = "/devices/31323334353637383930313233340000"
t = json.load(open("tree.json", encoding="utf-8"))
paths = [p for p, v in t.items() if p.startswith(DEV) and "/controls/" in p and "/effects/" not in p and (v.get("data") or {}).get("properties", {}).get("control_type")]
paths += [DEV] + [p for p in t if p.startswith(DEV + "/outputs/") or p.endswith("/jack_detect")]
out = open(sys.argv[1], "w", encoding="utf-8")
def log(*a): print(time.strftime("%H:%M:%S"), *a, file=out, flush=True)
s = socket.create_connection(("127.0.0.1", 4720)); s.settimeout(0.5)
for p in paths: s.sendall(f"subscribe {p}".encode() + b"\0")
log("subscribed", len(paths)); buf = b""; end = time.time() + float(sys.argv[2]); hb = 0
while time.time() < end:
    if time.time() - hb > 10: s.sendall(b"get /ping\0"); hb = time.time()
    try: d = s.recv(1 << 20)
    except socket.timeout: continue
    except OSError as e: log("SOCKET ERROR", e); break
    if not d: log("CLOSED BY SERVER"); break
    buf += d
    while b"\0" in buf:
        m, buf = buf.split(b"\0", 1); o = json.loads(m)
        if "error" in o: log("ERROR", o); continue
        if o.get("path") == "/ping" or o.get("data") is None: continue
        if o["path"] == "multi":
            md = o["data"]; names = [md["string"][i] for i in md["name"]]
            log("MULTI", "/".join(names[:9]), md["data"])
        else: log(o["path"].replace(DEV, "DEV"), json.dumps(o["data"])[:200])
log("END")
