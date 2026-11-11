import socket, time, json, sys
mode = sys.argv[1]
s = socket.create_connection(("127.0.0.1", 4720)); s.settimeout(0.5); t0 = time.time(); last = 0; buf = b""
s.sendall(b"subscribe /devices/31323334353637383930313233340000\0")
while time.time() - t0 < 40:
    if mode != "none" and time.time() - last > 8:
        s.sendall((mode.replace("_", " ") + "\0").encode()); last = time.time()
    try: d = s.recv(65536)
    except socket.timeout: continue
    except ConnectionError as e: print(round(time.time()-t0,1), "ERR", e); break
    if not d: print(round(time.time()-t0,1), "CLOSED by server"); break
    buf += d
    while b"\0" in buf:
        m, buf = buf.split(b"\0", 1); print(round(time.time()-t0,1), m[:150].decode())
else: print("alive after 40s")
