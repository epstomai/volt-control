"""Does the Volt preamp compressor do anything? Physical loopback MON out (one side) -> input 2 (48V off).

Finds which monitor side feeds input 2 at a low volume, then plays the tone on that side only, so the
speaker still on the other side gets nothing. Headphones muted during the test.
1) static: output level vs input level for OFF / FAST / GUITAR / VOCAL
2) dynamic: a -30 -> -6 -> -30 dBFS step, gain-reduction envelope (attack / release)
Safety: refuses with 48V on input 2 or any input-2 route (feedback) or playback already running;
input 1 DIRECT off during the test; monitor volume set so the loop is ~0 dB (user consent).
Everything is restored, compressor back to its original mode.
usage: python -I comp_test.py
"""
import json, math, socket, sys, threading, time, warnings
import numpy as np, sounddevice as sd

warnings.filterwarnings("ignore")
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
FS = 48000
B = "/devices/31323334353637383930313233340000/buses/"
IN1 = B + "8aa1d212e4bc4b328aed3c408174d447/"
IN2 = B + "0dda87cf8ccc4ca7957728c10401ec6a/"
MON = B + "d972b625d5d445249ca716ccf8829db2/outputs/main/controls/"
HP = B + "a7fef601f2c04340a813f379826985d7/outputs/main/controls/"
MODES = ["OFF", "FAST", "GUITAR", "VOCAL"]
CAPTURE = next(i for i, d in enumerate(sd.query_devices())
               if "MIC/LINE/HI-Z 1/2 (Volt 2 Gen 2)" in d["name"] and d["max_input_channels"] > 0
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


# ---- audio: tone generator with a level schedule, continuous capture
amp = [0.0]
ph = [0]
side = [0]  # output channel feeding input 2


def tone(out, n, *_):
    t = (np.arange(n) + ph[0]) / FS
    out[:] = 0
    out[:, side[0]] = (amp[0] * np.sin(2 * np.pi * 1000 * t)).astype(np.float32)
    ph[0] += n


cap, lock = [], threading.Lock()


def rec(x, n, *_):
    with lock:
        cap.append((time.time(), x[:, 1].copy()))  # input 2 = channel 2


def grab(t0, t1):
    with lock:
        return np.concatenate([c for t, c in cap if t0 <= t <= t1]).astype(np.float64)


def tone_db(x):
    """1 kHz component as dBFS peak (a full-scale sine reads 0)."""
    w = np.hanning(len(x))
    X = np.fft.rfft(x * w)
    k = int(round(1000 * len(x) / FS))
    a = math.sqrt(np.sum(np.abs(X[k - 3:k + 4]) ** 2) * 2 / (np.sum(w ** 2) * len(x))) * math.sqrt(2)
    return 20 * math.log10(a) if a > 0 else -math.inf


def level(settle=0.8, window=0.6):
    time.sleep(settle)
    t0 = time.time()
    time.sleep(window)
    return tone_db(grab(t0, time.time()))


def setamp(dbfs):
    amp[0] = 0.0 if dbfs is None else 10 ** (dbfs / 20)


# ---- preconditions and state
state0 = {"mon": get(MON + "gain"), "hp_mute": get(HP + "mute"), "mon_mute": get(MON + "mute"), "in1_direct_mute": get(IN1 + "outputs/main/controls/mute"),
          "in2_comp": get(IN2 + "controls/preamp_compressor"), "in2_gain": get(IN2 + "controls/preamp_gain")}
print("start:", state0)
if get(IN2 + "controls/preamp_48v"):
    raise SystemExit("input 2 has 48V on")
for o in ("main", "send_0", "send_1", "cue_0", "cue_1"):
    c = IN2 + f"outputs/{o}/controls/"
    if not (get(c + "mute") or get(c + "gain") <= -144):
        raise SystemExit(f"input 2 routes into '{o}' (feedback risk)")

out_stream = sd.OutputStream(samplerate=FS, channels=2, dtype="float32", callback=tone, blocksize=240)
in_stream = sd.InputStream(device=CAPTURE, samplerate=FS, channels=2, dtype="float32", callback=rec, blocksize=240)
static, dyn = {}, {}
try:
    put(HP + "mute", 1)  # headphones play the monitor mix too
    put(IN1 + "outputs/main/controls/mute", 1)  # mic out of the mix
    in_stream.start()
    out_stream.start()
    setamp(None)
    print(f"noise floor (tone bin): {level():.1f} dBFS")
    # find the monitor side wired to input 2 with a quiet tone, keep the user's volume and gain
    setamp(-60)
    got = []
    for sd_ in (0, 1):
        side[0] = sd_
        got.append(level())
        print(f"probe -60 dBFS on {'LR'[sd_]}: input 2 {got[-1]:.1f} dBFS", flush=True)
    setamp(None)
    side[0] = int(np.argmax(got))
    if got[side[0]] < -90 or got[side[0]] - got[1 - side[0]] < 20:
        raise SystemExit("no clean loop from one monitor side to input 2")
    print(f"monitor {'LR'[side[0]]} feeds input 2, loop gain {got[side[0]] + 60:.1f} dB", flush=True)
    LOUD = "loud" in sys.argv
    if LOUD:
        put(IN2 + "controls/preamp_gain", round(state0["in2_gain"] + 4, 1))  # recording only: DIRECT off, HP muted
    LEVELS = [-20, -14, -10, -8, -6, -4, -3, -2, -1, 0] if LOUD else [-50, -40, -30, -24, -18, -14, -10, -6]
    for i, mode in enumerate(MODES):
        put(IN2 + "controls/preamp_compressor", i)
        assert get(IN2 + "controls/preamp_compressor") == i, "compressor mode did not stick"
        row = []
        for L in LEVELS:
            setamp(L)
            row.append(level())
        static[mode] = row
        print(f"{mode:6s} " + "  ".join(f"{L:>4}->{o:6.1f}" for L, o in zip(LEVELS, row)), flush=True)
    # dynamic: -30 -> -6 (1.5 s) -> -30 (2 s)
    for i, mode in enumerate(MODES if not LOUD else []):
        put(IN2 + "controls/preamp_compressor", i)
        setamp(-30); time.sleep(1.5)
        t_up = time.time(); setamp(-6); time.sleep(1.5)
        t_dn = time.time(); setamp(-30); time.sleep(2.0)
        x = grab(t_up - 0.2, time.time())
        dyn[mode] = x
    setamp(None)
finally:
    out_stream.stop(); out_stream.close(); in_stream.stop(); in_stream.close()
    put(HP + "mute", int(state0["hp_mute"]))
    put(IN2 + "controls/preamp_compressor", state0["in2_comp"]); put(IN2 + "controls/preamp_gain", state0["in2_gain"])
    put(IN1 + "outputs/main/controls/mute", int(state0["in1_direct_mute"])); put(MON + "mute", int(state0["mon_mute"]))
    print("restored:", {"mon": get(MON + "gain"), "hp_mute": get(HP + "mute"), "mon_mute": get(MON + "mute"), "in1_direct_mute": get(IN1 + "outputs/main/controls/mute"),
                        "in2_comp": get(IN2 + "controls/preamp_compressor"), "in2_gain": get(IN2 + "controls/preamp_gain")}, "; tone stopped")

# envelope analysis: 5 ms RMS windows, relative to the OFF run
def env(x, win=240):
    n = len(x) // win
    r = np.sqrt(np.mean(x[: n * win].reshape(n, win) ** 2, axis=1))
    return 20 * np.log10(np.maximum(r, 1e-9))

np.savez("comp_test.npz", **{f"dyn_{k}": v for k, v in dyn.items()}, levels=np.array(LEVELS),
         **{f"static_{k}": np.array(v) for k, v in static.items()})
if "OFF" in dyn and len(dyn) == len(MODES):
    ref = env(dyn["OFF"])
    for mode in MODES[1:]:
        e = env(dyn[mode])
        n = min(len(e), len(ref))
        gr = e[:n] - ref[:n]  # gain reduction vs OFF, per 5 ms
        # find the step-up in the OFF envelope (first window 18 dB above the start)
        up = int(np.argmax(ref[:n] > ref[5] + 18))
        dn = up + int(np.argmax(ref[up + 20:n] < ref[up + 10] - 18)) + 20
        hold = gr[up + 200:dn - 10]
        steady = float(np.median(hold)) if len(hold) else float("nan")
        peak_over = float(np.max(gr[up:up + 40]))
        def t_to(seg, target):  # first window where gr is within 1 dB of target
            idx = np.where(np.abs(seg - target) < 1.0)[0]
            return idx[0] * 5 if len(idx) else None
        after = float(np.median(gr[n - 60:n]))
        print(f"{mode:6s}: steady GR at -6 dBFS {steady:6.1f} dB | attack to within 1 dB: {t_to(gr[up:dn], steady)} ms "
              f"| release to within 1 dB of {after:.1f}: {t_to(gr[dn:n], after)} ms")
