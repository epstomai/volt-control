"""Which props does UASrv push when the HP gain changes? Lowers HP by 1 step, then restores."""
import json, socket, time
G = "/devices/31323334353637383930313233340000/buses/a7fef601f2c04340a813f379826985d7/outputs/main/controls/gain"
s = socket.create_connection(("127.0.0.1", 4720)); s.settimeout(0.1); buf = b""
def send(m): s.sendall((m + "\0").encode())
def drain(sec):
    global buf; t = time.time(); out = []
    while time.time() - t < sec:
        try: buf += s.recv(1 << 20)
        except socket.timeout: pass
        while b"\0" in buf:
            m, buf = buf.split(b"\0", 1); out.append(json.loads(m.decode("utf-8", "replace")))
    return out
send(f"get {G}"); v0 = [o for o in drain(0.5) if o.get("path") == G][0]["data"]["properties"]["value"]["value"]
send(f"subscribe {G}"); drain(0.5)
send(f"set {G}/value/value {v0 - 1}")
for o in drain(1.0): print(str(o)[:160])
send(f"set {G}/value/value {v0}"); drain(0.5)
send(f"get {G}"); print("restored:", [o for o in drain(0.5) if o.get("path") == G][0]["data"]["properties"]["value"]["value"])
