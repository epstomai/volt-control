"""Which volume does the Volt LOOPBACK capture follow? Records 'LOOPBACK L/R' (WASAPI) while a
-20 dBFS 1 kHz tone plays, and varies HP (down only), MONITOR (muted) and LOOPBACK gain.
Everything is restored at the end.
usage: python -I loopback_probe.py probe
       python -I loopback_probe.py sweep <hp|loopback> [step] [out.csv]
"""
import json, math, socket, sys, threading, time
import numpy as np, sounddevice as sd

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
B = "/devices/31323334353637383930313233340000/buses/"
LB = B + "b0dc87cf068741b5933ebb676712dc6b/outputs/main/controls/"
MON = B + "d972b625d5d445249ca716ccf8829db2/outputs/main/controls/"
HP = B + "a7fef601f2c04340a813f379826985d7/outputs/main/controls/"
CAPTURE = next(i for i, d in enumerate(sd.query_devices())
               if "LOOPBACK L/R (Volt 2 Gen 2)" in d["name"] and d["max_input_channels"] > 0
               and sd.query_hostapis(d["hostapi"])["name"] == "Windows WASAPI")

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


blocks, lock = [], threading.Lock()


def rec(indata, n, *_):
    with lock:
        blocks.append((time.time(), float(np.sqrt(np.mean(indata.astype(np.float64) ** 2)))))


def level(settle=0.5, window=0.6):
    """RMS of the capture, in dBFS (a full-scale sine reads -3.0)."""
    time.sleep(settle)
    t0 = time.time()
    time.sleep(window)
    with lock:
        w = [r for t, r in blocks if t >= t0]
    r = float(np.median(w)) if w else 0.0
    return 20 * math.log10(r) if r > 0 else -math.inf


ph = [0]


def tone(out, n, *_):
    t = (np.arange(n) + ph[0]) / 48000
    out[:] = (0.1 * np.sin(2 * np.pi * 1000 * t)).astype(np.float32)[:, None]
    ph[0] += n


state0 = {"lb_gain": get(LB + "gain"), "lb_mute": get(LB + "mute"), "mon_gain": get(MON + "gain"),
          "mon_mute": get(MON + "mute"), "hp_gain": get(HP + "gain")}
print("start:", state0)
mode = sys.argv[1] if len(sys.argv) > 1 else "probe"
out_stream = sd.OutputStream(samplerate=48000, channels=2, dtype="float32", callback=tone)
in_stream = sd.InputStream(device=CAPTURE, samplerate=48000, channels=2, dtype="float32", callback=rec)
rows = []
try:
    put(MON + "mute", 1)  # speakers stay silent whatever happens below
    in_stream.start()
    print(f"no tone                         : {level():7.2f} dBFS")
    out_stream.start()
    if mode == "probe":
        print(f"tone, as found                  : {level():7.2f} dBFS")
        put(HP + "gain", round(state0["hp_gain"] / 2, 1))
        print(f"tone, HP {round(state0['hp_gain'] / 2, 1):<5}                  : {level():7.2f} dBFS")
        put(HP + "gain", state0["hp_gain"])
        put(MON + "gain", 100)
        print(f"tone, MON 100 (muted)           : {level():7.2f} dBFS")
        put(MON + "gain", state0["mon_gain"])
        for g in (0, 50, 100):
            put(LB + "gain", g)
            print(f"tone, LOOPBACK gain {g:<3}         : {level():7.2f} dBFS")
    else:
        which = sys.argv[2]
        step = float(sys.argv[3]) if len(sys.argv) > 3 else 2
        target, start = (HP + "gain", state0["hp_gain"]) if which == "hp" else (LB + "gain", 100.0)
        v = start
        while v >= -1e-9:
            put(target, round(v, 3))
            lv = level()
            rows.append((round(v, 3), lv))
            print(f"{which} {v:6.1f}  loopback capture {lv:7.2f} dBFS", flush=True)
            v -= step
finally:
    out_stream.stop(); out_stream.close(); in_stream.stop(); in_stream.close()
    put(HP + "gain", state0["hp_gain"]); put(MON + "gain", state0["mon_gain"]); put(MON + "mute", int(state0["mon_mute"]))
    put(LB + "gain", state0["lb_gain"]); put(LB + "mute", int(state0["lb_mute"]))
    print("restored:", {"lb_gain": get(LB + "gain"), "lb_mute": get(LB + "mute"), "mon_gain": get(MON + "gain"),
                        "mon_mute": get(MON + "mute"), "hp_gain": get(HP + "gain")}, "; tone stopped")

if rows:
    out = sys.argv[4] if len(sys.argv) > 4 else f"{sys.argv[2]}_taper_loopback.csv"
    with open(out, "w", encoding="utf-8") as f:
        f.write("volume,capture_rms_dbfs\n")
        for v, lv in rows:
            f.write(f"{v},{lv:.3f}\n")
    print("saved", out)
