"""Compare the input-1 'Input Metering' (units=percent) pushes with the real capture level, block by block."""
import json, socket, time, math, threading
import numpy as np, sounddevice as sd
M = "/devices/31323334353637383930313233340000/buses/8aa1d212e4bc4b328aed3c408174d447/outputs/main/meters/level"
dev = next(i for i, d in enumerate(sd.query_devices()) if "MIC/LINE/HI-Z 1/2 (Volt 2 Gen 2)" in d["name"] and d["max_input_channels"] > 0 and sd.query_hostapis(d["hostapi"])["name"] == "Windows WASAPI")
s = socket.create_connection(("127.0.0.1", 4720)); s.settimeout(0.05); buf = b""
meter, cap, lock = [], [], threading.Lock()
def cb(x, n, *_):
    with lock: cap.append((time.time(), float(np.max(np.abs(x[:, 0]))), float(np.sqrt(np.mean(x[:, 0].astype(np.float64) ** 2)))))
st = sd.InputStream(device=dev, samplerate=48000, channels=2, dtype="float32", blocksize=960, callback=cb); st.start()
s.sendall(f"subscribe {M}\0".encode()); t0 = time.time()
while time.time() - t0 < 8:
    try: buf += s.recv(1 << 20)
    except socket.timeout: pass
    while b"\0" in buf:
        m, buf = buf.split(b"\0", 1); o = json.loads(m.decode("utf-8", "replace"))
        if o.get("path") == "multi" and o["data"].get("tag") == "meters":
            d = o["data"]; stt = d["string"]; row = {}
            for j, i in enumerate(d["path"]): row[stt[d["name"][d["parent"][i]]]] = d["data"][j][0]
            meter.append((time.time(), row.get("level"), row.get("peak")))
st.stop()
db = lambda v: 20 * math.log10(v) if v and v > 0 else -200
pairs = []
for t, lv, pk in meter:
    w = [c for c in cap if t - 0.06 <= c[0] <= t]
    if w and lv is not None:
        pairs.append((lv, pk, max(c[1] for c in w), math.sqrt(np.mean([c[2] ** 2 for c in w]))))
pairs.sort()
print(len(meter), "pushes;", len(pairs), "pairs")
for k in range(0, len(pairs), max(1, len(pairs) // 14)):
    lv, pk, cpk, crms = pairs[k]
    print(f"meter level {lv:.4f} ({db(lv):6.1f} dB if linear)  peak {pk:.4f} | capture peak {db(cpk):6.1f} dBFS rms {db(crms):6.1f} dBFS")
print("overall: meter peak max", round(db(max(p for _, _, p in meter if p is not None)), 1), "dB(lin) | capture peak max", round(db(max(c[1] for c in cap)), 1), "dBFS")
# lag search: meter peak vs capture peak, both as time series of block maxima
