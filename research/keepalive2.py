"""Does UASrv drop idle clients? usage: python -I keepalive2.py <none|get_/ping> <secs> [interval]"""
import socket, sys, time
mode, dur = sys.argv[1], float(sys.argv[2])
every = float(sys.argv[3]) if len(sys.argv) > 3 else 20
s = socket.create_connection(("127.0.0.1", 4720)); s.settimeout(0.5)
t0 = time.time(); last = time.time(); n = 0
s.sendall(b"subscribe /devices/31323334353637383930313233340000\0")
while time.time() - t0 < dur:
    if mode != "none" and time.time() - last > every:
        s.sendall((mode.replace("_", " ") + "\0").encode()); last = time.time()
    try:
        d = s.recv(65536)
    except socket.timeout:
        continue
    except ConnectionError as e:
        print(f"{time.time()-t0:.1f} ERR {e}", flush=True); break
    if not d:
        print(f"{time.time()-t0:.1f} CLOSED by server after {n} msgs", flush=True); break
    n += d.count(b"\0")
else:
    print(f"alive after {dur}s, {n} msgs")
