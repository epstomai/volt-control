"""Subscribe to all controls of the Volt and log every pushed change (for hardware-knob mapping)."""
import socket, json, time, sys
DEV = "/devices/31323334353637383930313233340000"
t = json.load(open("tree.json", encoding="utf-8"))
paths = [p for p, v in t.items() if p.startswith(DEV) and "/controls/" in p and "/effects/" not in p and (v.get("data") or {}).get("properties", {}).get("control_type")]
paths += [DEV] + [p for p in t if p.startswith(DEV + "/outputs/") or p.endswith("/jack_detect")]
s = socket.create_connection(("127.0.0.1", 4720)); s.settimeout(1)
for p in paths: s.sendall(f"subscribe {p}".encode() + b"\0")
out = open(sys.argv[1], "w", encoding="utf-8"); buf = b""; end = time.time() + float(sys.argv[2]); last_ping = time.time()
print(f"subscribed {len(paths)} paths", file=out, flush=True)
while time.time() < end:
    if time.time() - last_ping > 10: s.sendall(b"get /ping\0"); last_ping = time.time()
    try: d = s.recv(1 << 20)
    except socket.timeout: continue
    if not d: break
    buf += d
    while b"\0" in buf:
        m, buf = buf.split(b"\0", 1); o = json.loads(m)
        if o.get("data") is None or o.get("path") in ("/ping",): continue
        if o["path"] == "multi":
            md = o["data"]; strs = md["string"]
            print(time.strftime("%H:%M:%S"), "MULTI", json.dumps(md)[:400], file=out, flush=True)
        else:
            print(time.strftime("%H:%M:%S"), o["path"].replace(DEV, "DEV"), json.dumps(o["data"])[:200], file=out, flush=True)
