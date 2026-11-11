"""Calibrate UASrv 'percent' meters with known sine levels played to the default output (monitor volume 0)."""
import json, socket, time, math
import numpy as np, sounddevice as sd
B = "/devices/31323334353637383930313233340000/buses/"
MS = {"loopback": B + "b0dc87cf068741b5933ebb676712dc6b/outputs/main/meters/level", "monitor": B + "d972b625d5d445249ca716ccf8829db2/outputs/main/meters/level"}
s = socket.create_connection(("127.0.0.1", 4720)); s.settimeout(0.05); buf = b""
def get(p):
    global buf
    s.sendall(f"get {p}\0".encode())
    while True:
        while b"\0" not in buf: buf += s.recv(1 << 20)
        m, buf = buf.split(b"\0", 1); o = json.loads(m.decode("utf-8", "replace"))
        if o.get("path") == p: return o["data"]["properties"]
mon = get(B + "d972b625d5d445249ca716ccf8829db2/outputs/main/controls/gain")["value"]["value"]
assert mon == 0, f"monitor volume is {mon}, refusing to play"
for k, p in MS.items(): print(k, "units:", get(p)["units"]["value"])
for p in MS.values(): s.sendall(f"subscribe {p}\0".encode())
def read(sec):
    global buf
    out = {k: [] for k in MS}; t = time.time()
    while time.time() - t < sec:
        try: buf += s.recv(1 << 20)
        except socket.timeout: pass
        while b"\0" in buf:
            m, buf = buf.split(b"\0", 1); o = json.loads(m.decode("utf-8", "replace"))
            if o.get("path") == "multi" and o["data"].get("tag") == "meters":
                d = o["data"]; st = d["string"]
                for k, p in MS.items():
                    if p.split("/")[4] in st:
                        for j, i in enumerate(d["path"]):
                            if st[d["name"][d["parent"][i]]] == "level": out[k].append(max(d["data"][j]))
    return {k: (float(np.min(v)), float(np.median(v)), float(np.max(v))) if v else None for k, v in out.items()}
amp = [0.0]; ph = [0]
def cb(o, n, *_):
    t = (np.arange(n) + ph[0]) / 48000; o[:] = (amp[0] * np.sin(2 * np.pi * 1000 * t)).astype(np.float32)[:, None]; ph[0] += n
st = sd.OutputStream(samplerate=48000, channels=2, dtype="float32", callback=cb); st.start()
try:
    for dbfs in [-1, -3, -6, -9, -12, -15, -18, -21, -24, -27, -30, -35, -40, -45, -50, -55, -60, -70, -80, None]:
        amp[0] = 0.0 if dbfs is None else 10 ** (dbfs / 20); read(1.2)
        r = read(0.6)
        print(f"sine {'off' if dbfs is None else f'{dbfs} dBFS peak':>14}: " + "  ".join(f"{k} {v[1]:.4f} [{v[0]:.4f}..{v[2]:.4f}]" if v else f"{k} -" for k, v in r.items()), flush=True)
finally:
    st.stop(); st.close(); print("tone stopped")
