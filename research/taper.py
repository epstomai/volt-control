"""Read-only: find MONITOR / HEADPHONE output gain controls and sample their taper via the
get_real_for_normalized_value / get_string_for_normalized_value commands (no value is changed)."""
import json, socket
DEV = "/devices/31323334353637383930313233340000"
s = socket.create_connection(("127.0.0.1", 4720), timeout=5); buf = b""
def call(line, path):
    global buf
    s.sendall(line.encode() + b"\0")
    while True:
        while b"\0" not in buf: buf += s.recv(1 << 20)
        m, buf = buf.split(b"\0", 1)
        o = json.loads(m.decode("utf-8", "replace"))
        if o.get("path") == path: return o
get = lambda p: call(f"get {p}", p).get("data") or {}
for ch in get(DEV + "/buses")["children"]:
    bp = f"{DEV}/buses/{ch['path']}"
    props = get(bp).get("properties", {})
    name = props.get("name", {}).get("value"); btype = props.get("bus_type", {}).get("value")
    if name not in ("MONITOR", "HEADPHONE", "Monitor", "Headphone") and btype not in ("master",):
        continue
    g = f"{bp}/outputs/main/controls/gain"
    gp = get(g).get("properties", {})
    print(f"== {name} ({btype}) value={gp.get('value',{}).get('value')} norm={gp.get('normalized_value',{}).get('value')} str={gp.get('string_value',{}).get('value')} taper={gp.get('taper',{}).get('value')}")
    for n in [i / 20 for i in range(21)]:
        r = call(f"post {g}/get_real_for_normalized_value {json.dumps({'normalized_value': n})}", f"{g}/get_real_for_normalized_value")
        t = call(f"post {g}/get_string_for_normalized_value {json.dumps({'normalized_value': n})}", f"{g}/get_string_for_normalized_value")
        print(f"  n={n:.2f}  real={r.get('data', r.get('error'))}  str={t.get('data', t.get('error'))}")
