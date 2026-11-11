"""Read-only: GET /status from UAMixerHelper (127.0.0.1:4720) and look up feature keys by name (FNV-1a 64 decimal for hashed keys).
usage: python -I fk_live.py <name> [...]"""
import json, socket, sys, time

def fnv1a64(s):
    h = 0xcbf29ce484222325
    for c in s.encode():
        h ^= c
        h = (h * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return h

s = socket.create_connection(("127.0.0.1", 4720), timeout=3)
s.sendall(b"get /status\0")
buf = b""; end = time.time() + 4
s.settimeout(0.5)
while time.time() < end:
    try:
        d = s.recv(1 << 20)
        if not d: break
        buf += d
        if b"\0" in buf: break
    except socket.timeout:
        pass
msg = json.loads(buf.split(b"\0")[0])
props = msg["data"]["properties"]
fk = props["feature_keys"]["value"]
print("config =", json.dumps(props["config"]["value"]))
print("total keys =", len([k for k in fk if not k.endswith("-desc")]))
for n in sys.argv[1:]:
    h = str(fnv1a64(n))
    if n in fk:
        print(f"{n}: plain key, value={fk[n]!r}")
    elif h in fk:
        print(f"{n}: hashed key {h}, value={fk[h]!r}")
    else:
        print(f"{n}: not present (hash {h})")
