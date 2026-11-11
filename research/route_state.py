"""Read-only: list every Volt bus with its preamp settings and the gain/mute of each output send.
usage: python -I route_state.py"""
import json, socket, sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
DEV = "/devices/31323334353637383930313233340000"
s = socket.create_connection(("127.0.0.1", 4720), timeout=5); buf = b""


def get(p):
    global buf
    s.sendall(f"get {p}\0".encode())
    while True:
        while b"\0" not in buf:
            buf += s.recv(1 << 20)
        m, buf = buf.split(b"\0", 1)
        o = json.loads(m.decode("utf-8", "replace"))
        if o.get("path") == p:
            return o.get("data") or {}


def children(p):
    return [c["path"] for c in get(p).get("children") or []]


def prop(d, k):
    v = (d.get("properties") or {}).get(k, {})
    return v.get("value") if isinstance(v, dict) else v


def ctl(p):
    d = get(p)
    return prop(d, "value") if d else None


for b in children(DEV + "/buses"):
    bp = f"{DEV}/buses/{b}"
    d = get(bp)
    print(f"\n[{prop(d, 'bus_type')}] {prop(d, 'name')}  enabled={prop(d, 'enabled')}  {b}")
    ctls = children(bp + "/controls")
    pre = {c: ctl(f"{bp}/controls/{c}") for c in ctls if c.startswith("preamp") or c in ("solo", "input_select")}
    if pre:
        print("   ", pre)
    for o in children(bp + "/outputs"):
        oc = children(f"{bp}/outputs/{o}/controls")
        vals = {c: ctl(f"{bp}/outputs/{o}/controls/{c}") for c in oc if c in ("gain", "mute", "enabled", "pre_fader")}
        print(f"    -> {o:8s} {vals}")
