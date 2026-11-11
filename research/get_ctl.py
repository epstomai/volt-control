"""Read-only: dump properties of a UASrv object. usage: python -I get_ctl.py <path>..."""
import json, socket, sys
s = socket.create_connection(("127.0.0.1", 4720), timeout=3); buf = b""
for path in sys.argv[1:]:
    s.sendall(f"get {path}\0".encode())
    while True:
        while b"\0" not in buf: buf += s.recv(1 << 20)
        m, buf = buf.split(b"\0", 1)
        o = json.loads(m.decode("utf-8", "replace"))
        if o.get("path") == path: break
    print("==", path); print(json.dumps(o.get("data", o), ensure_ascii=False, indent=1)[:4000])
