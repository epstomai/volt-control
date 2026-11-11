import socket, json, time
DEV = "/devices/31323334353637383930313233340000"
HP = DEV + "/buses/a7fef601f2c04340a813f379826985d7/outputs/main/controls/gain"
s = socket.create_connection(("127.0.0.1", 4720)); s.settimeout(1)
s.sendall(f"subscribe {HP}\0".encode()); s.sendall(f"subscribe {DEV}\0".encode()); time.sleep(0.3)
c = socket.create_connection(("127.0.0.1", 4720)); c.sendall(f"set {HP}/value 100\0".encode())
buf = b""; end = time.time() + 1.5
while time.time() < end:
    try: buf += s.recv(1 << 20)
    except socket.timeout: pass
for m in buf.split(b"\0"):
    if m: print(m.decode()); print()
