"""Measure the HEADPHONE volume taper through a physical loopback: HP out -> input 2 (line, 48V off).

Plays a -20 dBFS 1 kHz tone (left / right / both) to the default output and records the Volt
'MIC/LINE/HI-Z 1/2' capture. Level = 1 kHz component (FFT, Hann), so it reads well below the noise.
Safety: aborts if input 2 has 48V or any route back into a mix (feedback); monitor muted throughout;
input 2 preamp gain lowered to its minimum; HP volume only goes DOWN from where it is; all restored.
usage: python -I hp_loop.py probe
       python -I hp_loop.py sweep [step] [out.csv] [L|R|LR]
"""
import json, math, socket, sys, threading, time
import numpy as np, sounddevice as sd

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
FS = 48000
B = "/devices/31323334353637383930313233340000/buses/"
IN2 = B + "0dda87cf8ccc4ca7957728c10401ec6a/"
MON = B + "d972b625d5d445249ca716ccf8829db2/outputs/main/controls/"
HP = B + "a7fef601f2c04340a813f379826985d7/outputs/main/controls/"
CAPTURE = next(i for i, d in enumerate(sd.query_devices())
               if "MIC/LINE/HI-Z 1/2 (Volt 2 Gen 2)" in d["name"] and d["max_input_channels"] > 0
               and sd.query_hostapis(d["hostapi"])["name"] == "Windows WASAPI")
NCH = sd.query_devices(CAPTURE)["max_input_channels"]

s = socket.create_connection(("127.0.0.1", 4720), timeout=5); buf = b""


def call(v, p, a=None):
    global buf
    s.sendall(f"{v} {p}{'' if a is None else ' ' + a}\0".encode())
    while True:
        while b"\0" not in buf:
            buf += s.recv(1 << 20)
        m, buf = buf.split(b"\0", 1)
        o = json.loads(m.decode("utf-8", "replace"))
        if o.get("path") == p:
            return o


def get(p):
    return call("get", p)["data"]["properties"]["value"]["value"]


def put(p, v):
    call("set", p + "/value/value", f"{v}")


chunks, lock = [], threading.Lock()


def rec(indata, n, *_):
    with lock:
        chunks.append((time.time(), indata.copy()))


def level(settle=1.2, window=1.0):
    """per capture channel: (1 kHz component dBFS-RMS, broadband RMS dBFS, peak dBFS)."""
    time.sleep(settle)
    t0 = time.time()
    time.sleep(window)
    with lock:
        x = np.concatenate([c for t, c in chunks if t >= t0]).astype(np.float64)
        chunks.clear()
    w = np.hanning(len(x))
    out = []
    for ch in range(x.shape[1]):
        X = np.fft.rfft(x[:, ch] * w)
        k = int(round(1000 * len(x) / FS))
        amp = math.sqrt(np.sum(np.abs(X[k - 3:k + 4]) ** 2) * 2 / (np.sum(w ** 2) * len(x))) # RMS of the tone
        rms = math.sqrt(np.mean(x[:, ch] ** 2))
        pk = np.max(np.abs(x[:, ch]))
        f = lambda v: 20 * math.log10(v) if v > 0 else -math.inf
        out.append((f(amp), f(rms), f(pk)))
    return out


chan = {"L": (1, 0), "R": (0, 1), "LR": (1, 1)}
mask = [chan["L"]]
ph = [0]


def tone(out, n, *_):
    t = (np.arange(n) + ph[0]) / FS
    x = (0.1 * np.sin(2 * np.pi * 1000 * t)).astype(np.float32)
    out[:, 0] = x * mask[0][0]
    out[:, 1] = x * mask[0][1]
    ph[0] += n


def show(label, lv):
    print(f"{label:24s}" + "  |  ".join(f"ch{i + 1}: tone {a:7.2f}  rms {r:7.2f}  pk {p:6.1f}" for i, (a, r, p) in enumerate(lv)), flush=True)


state0 = {"hp": get(HP + "gain"), "mon_gain": get(MON + "gain"), "mon_mute": get(MON + "mute"),
          "in2_gain": get(IN2 + "controls/preamp_gain"), "in2_48v": get(IN2 + "controls/preamp_48v")}
print("start:", state0, "| capture device", CAPTURE, sd.query_devices(CAPTURE)["name"], f"{NCH}ch")
if state0["in2_48v"]:
    raise SystemExit("input 2 has 48V on: switch it off on the Volt first (phantom into the HP out)")
for o in ("main", "send_0", "send_1", "cue_0", "cue_1"):
    c = IN2 + f"outputs/{o}/controls/"
    if not (get(c + "mute") or get(c + "gain") <= -144):
        raise SystemExit(f"input 2 routes into '{o}' (feedback risk): turn DIRECT off / sends down first")

mode = sys.argv[1] if len(sys.argv) > 1 else "probe"
out_stream = sd.OutputStream(samplerate=FS, channels=2, dtype="float32", callback=tone)
in_stream = sd.InputStream(device=CAPTURE, samplerate=FS, channels=NCH, dtype="float32", callback=rec)
rows = []
try:
    put(MON + "mute", 1)
    put(IN2 + "controls/preamp_gain", 0)
    print("input 2 preamp gain ->", get(IN2 + "controls/preamp_gain"))
    in_stream.start()
    show("no tone", level())
    out_stream.start()
    if mode == "probe":
        for m in ("L", "R", "LR"):
            mask[0] = chan[m]
            show(f"tone {m:2s} HP {state0['hp']}", level())
        mask[0] = chan["L"]
        half = round(state0["hp"] / 2, 1)
        put(HP + "gain", half)
        show(f"tone L  HP {half}", level())
    else:
        step = float(sys.argv[2]) if len(sys.argv) > 2 else 2
        mask[0] = chan[sys.argv[4] if len(sys.argv) > 4 else "L"]
        v = state0["hp"]
        while v >= -1e-9:
            put(HP + "gain", round(v, 3))
            lv = level()
            rows.append((round(v, 3), lv))
            show(f"HP {v:6.1f}", lv)
            v -= step
finally:
    out_stream.stop(); out_stream.close(); in_stream.stop(); in_stream.close()
    put(HP + "gain", state0["hp"]); put(IN2 + "controls/preamp_gain", state0["in2_gain"])
    put(MON + "gain", state0["mon_gain"]); put(MON + "mute", int(state0["mon_mute"]))
    print("restored:", {"hp": get(HP + "gain"), "mon_gain": get(MON + "gain"), "mon_mute": get(MON + "mute"),
                        "in2_gain": get(IN2 + "controls/preamp_gain")}, "; tone stopped")

if rows:
    out = sys.argv[3] if len(sys.argv) > 3 else "hp_taper_loop.csv"
    with open(out, "w", encoding="utf-8") as f:
        f.write("hp_volume," + ",".join(f"ch{i + 1}_tone_dbfs,ch{i + 1}_rms_dbfs" for i in range(NCH)) + "\n")
        for v, lv in rows:
            f.write(f"{v}," + ",".join(f"{a:.3f},{r:.3f}" for a, r, _ in lv) + "\n")
    print("saved", out)
