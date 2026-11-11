"""Measure the MONITOR volume taper with the monitor MUTED (user switched the speakers off too).

Plays a -20 dBFS 1 kHz tone, checks that the MONITOR output meter follows the volume, then steps
the volume 0 -> 100 and records the meter (peak and value leaves). Always restores the original
volume and mute state and stops the tone.
usage: python -I sweep_mon.py [step] [out.csv]
"""
import json, math, socket, statistics, sys, time
import numpy as np, sounddevice as sd

BUS = "/devices/31323334353637383930313233340000/buses/d972b625d5d445249ca716ccf8829db2"
GAIN, MUTE, METER = BUS + "/outputs/main/controls/gain", BUS + "/outputs/main/controls/mute", BUS + "/outputs/main/meters/level"
STEP = float(sys.argv[1]) if len(sys.argv) > 1 else 2
OUT = sys.argv[2] if len(sys.argv) > 2 else "mon_taper.csv"
POST = "--post-fader" in sys.argv  # temporarily switch meters to post-fader
PF = "/post_fader_meters"

s = socket.create_connection(("127.0.0.1", 4720)); s.settimeout(0.05); buf = b""; replies = {}


def leaves(d):
    """multi meter packet -> {leaf kind ('peak'/'value'/'clip'): per-channel list}"""
    st, names, parents = d["string"], d["name"], d["parent"]
    out = {}
    for j, i in enumerate(d["path"]):
        out[st[names[parents[i]]]] = d["data"][j]  # the leaf is ".../<kind>/value"
    return out


def pump(secs, sink=None):
    global buf
    t0 = time.time()
    while time.time() - t0 < secs:
        try:
            buf += s.recv(1 << 20)
        except socket.timeout:
            pass
        while b"\0" in buf:
            m, buf = buf.split(b"\0", 1)
            o = json.loads(m.decode("utf-8", "replace"))
            if o.get("path") == "multi":
                if sink is not None and o["data"].get("tag") == "meters" and BUS[-32:] in o["data"]["string"]:
                    sink.append(leaves(o["data"]))
            else:
                replies[o.get("path")] = o


def call(v, p, a=None):
    replies.pop(p, None)
    s.sendall(f"{v} {p}{'' if a is None else ' ' + a}\0".encode())
    for _ in range(250):
        pump(0.02)
        if p in replies:
            return replies.pop(p)
    raise RuntimeError(f"no reply to {v} {p}")


def val(p):
    return call("get", p)["data"]["properties"]["value"]["value"]


def db(x):
    return 20 * math.log10(x) if x > 0 else -math.inf


def measure(settle=0.45, window=0.5):
    pump(settle)
    w = []
    pump(window, w)
    if not w:
        return -math.inf, -math.inf, 0
    peak = statistics.median(db(max(x.get("peak", [0]))) for x in w)
    level = statistics.median(db(max(x.get("value", [0]))) for x in w)
    return peak, level, len(w)


g0, m0 = val(GAIN), val(MUTE)
pf0 = call("get", "/")["data"]["properties"]["post_fader_meters"]["value"]
print(f"start: monitor volume {g0}, mute {m0}, post_fader_meters {pf0}")
ph = [0]


def cb(out, n, *_):
    t = (np.arange(n) + ph[0]) / 48000
    out[:] = (0.1 * np.sin(2 * np.pi * 1000 * t)).astype(np.float32)[:, None]
    ph[0] += n


rows = []
st = sd.OutputStream(samplerate=48000, channels=2, dtype="float32", callback=cb)
try:
    call("set", MUTE + "/value/value", "1")
    if val(MUTE) != 1:
        raise SystemExit("could not mute the monitor; aborting before any tone")
    call("set", GAIN + "/value/value", "0")
    if POST:
        call("set", PF + "/value", "true")
        print("post_fader_meters ->", call("get", "/")["data"]["properties"]["post_fader_meters"]["value"])
    call("subscribe", METER)
    print("no tone, vol 0:   peak %.1f value %.1f (%d pushes)" % measure())
    st.start()
    lo = measure()
    print("tone, vol 0:      peak %.1f value %.1f (%d pushes)" % lo)
    call("set", GAIN + "/value/value", "100")
    hi = measure()
    print("tone, vol 100:    peak %.1f value %.1f (%d pushes)" % hi)
    if not (math.isfinite(hi[0]) and (not math.isfinite(lo[0]) or hi[0] - lo[0] > 6)):
        raise SystemExit("meter does not follow the volume (pre-volume meter); no sweep")
    k = 0
    while k * STEP <= 100:
        v = round(k * STEP, 3)
        call("set", GAIN + "/value/value", f"{v}")
        peak, level, n = measure()
        rows.append((v, peak, level))
        print(f"vol {v:6.1f}  peak {peak:7.2f}  value {level:7.2f}  ({n})", flush=True)
        k += 1
finally:
    st.stop(); st.close()
    call("set", GAIN + "/value/value", f"{g0}")
    call("set", MUTE + "/value/value", f"{int(m0)}")
    call("set", PF + "/value", "true" if pf0 else "false")
    print(f"restored: monitor volume {val(GAIN)}, mute {val(MUTE)}, post_fader_meters {call('get', '/')['data']['properties']['post_fader_meters']['value']}; tone stopped")

if rows:
    with open(OUT, "w", encoding="utf-8") as f:
        f.write("volume,peak_dbfs,value_dbfs\n")
        for v, p, l in rows:
            f.write(f"{v},{p:.3f},{l:.3f}\n")
    print("saved", OUT)
