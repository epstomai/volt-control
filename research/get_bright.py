"""Prints the Volt's display_brightness and Volt Control's saved screen-sleep state (read only)."""
import json, os, socket
s = socket.create_connection(("127.0.0.1", 4720), timeout=5)
p = "/devices/31323334353637383930313233340000"
s.sendall(f"get {p}\0".encode()); buf = b""
while True:
    while b"\0" not in buf: buf += s.recv(1 << 20)
    m, buf = buf.split(b"\0", 1); o = json.loads(m.decode("utf-8", "replace"))
    if o.get("path") == p: break
c = json.load(open(os.path.expandvars(r"%APPDATA%\com.knia.voltcontrol\config.json"), encoding="utf-8"))
print("display_brightness =", o["data"]["properties"]["display_brightness"]["value"], "| dimmed_from =", c.get("screen_dimmed_from"))
