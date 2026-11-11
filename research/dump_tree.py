"""Recursively dump the UASrv parameter tree from UAMixerHelper (127.0.0.1:4720)."""
import json, socket, sys, time

class Conn:
    def __init__(self, port=4720):
        self.s = socket.create_connection(("127.0.0.1", port), timeout=5)
        self.buf = b""; self.other = []
    def get(self, path):
        self.s.sendall(f"get {path}".encode() + b"\0")
        while True:
            while b"\0" not in self.buf:
                self.buf += self.s.recv(1 << 20)
            msg, self.buf = self.buf.split(b"\0", 1)
            obj = json.loads(msg)
            if obj.get("path") == path:
                return obj
            self.other.append(obj)
            # unsolicited message (update) - ignore

def walk(c, path, out, depth=0, maxdepth=14):
    try:
        r = c.get(path)
    except TimeoutError:
        print("TIMEOUT", path, "| last other:", [o.get("path") for o in c.other[-3:]]); c.buf = b""; out[path] = {"timeout": True}; return
    out[path] = r
    if depth >= maxdepth: return
    for ch in (r.get("data") or {}).get("children", []) or []:
        p = ch["path"]
        walk(c, path.rstrip("/") + "/" + p, out, depth + 1)

root = sys.argv[1] if len(sys.argv) > 1 else "/"
c = Conn(); out = {}
t = time.time(); walk(c, root, out)
json.dump(out, open(sys.argv[2] if len(sys.argv) > 2 else "tree.json", "w", encoding="utf-8"), indent=1, ensure_ascii=False)
print(len(out), "nodes", round(time.time() - t, 1), "s")
