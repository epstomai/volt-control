import socket, sys, time
port = int(sys.argv[1]); cmds = sys.argv[2:]
s = socket.create_connection(("127.0.0.1", port), timeout=2)
for c in cmds:
    s.sendall(c.encode() + b"\0")
buf = b""; end = time.time() + 2.5
s.settimeout(0.5)
while time.time() < end:
    try:
        d = s.recv(65536)
        if not d: break
        buf += d
    except socket.timeout:
        pass
print(len(buf)); print(buf[:4000].replace(b"\0", b"\n<NUL>\n").decode("utf-8", "replace"))
