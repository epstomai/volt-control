import socket, json, time, collections
DEV = "/devices/31323334353637383930313233340000"
G = DEV + "/buses/8aa1d212e4bc4b328aed3c408174d447/controls/preamp_gain"
t = json.load(open("tree.json", encoding="utf-8"))
paths = [p for p, v in t.items() if p.startswith(DEV) and "/controls/" in p and "/effects/" not in p and (v.get("data") or {}).get("properties", {}).get("control_type")]
s = socket.create_connection(("127.0.0.1", 4720)); s.settimeout(0.3)
for p in paths: s.sendall(f"subscribe {p}".encode() + b"\0")
buf = b""; res = collections.Counter(); errs = []
def pump(sec):
    global buf; out = []; end = time.time() + sec
    while time.time() < end:
        try: d = s.recv(1 << 20)
        except socket.timeout: continue
        if not d: out.append("CLOSED"); break
        buf += d
        while b"\0" in buf:
            m, buf = buf.split(b"\0", 1); out.append(json.loads(m))
    return out
first = pump(2)
for o in first:
    if isinstance(o, dict) and "error" in o: errs.append((o["path"][-40:], o["error"]))
    res["error" if isinstance(o, dict) and "error" in o else "ok"] += 1
print(len(paths), "subs ->", res, errs[:3])
c2 = socket.create_connection(("127.0.0.1", 4720))
c2.sendall(f"set {G}/value 45.4".encode() + b"\0")   # same value: no-op
c2.sendall(f"set {DEV}/display_brightness 22".encode() + b"\0")
for o in pump(1.5): print("PUSH", json.dumps(o)[:200])
