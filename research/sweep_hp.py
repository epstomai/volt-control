"""Measure the HEADPHONE volume taper: play a 1 kHz sine, step the HP gain 100 -> 0, read meters.

Only lowers the HP volume from its current value and always restores it (and stops the tone).
usage: python -I sweep_hp.py [step] [out.csv]
"""
import json
import math
import socket
import statistics
import sys
import threading
import time

import numpy as np
import sounddevice as sd

DEV = "/devices/31323334353637383930313233340000"
HP_BUS = DEV + "/buses/a7fef601f2c04340a813f379826985d7"
GAIN = HP_BUS + "/outputs/main/controls/gain"
METERS = [HP_BUS + "/outputs/main/meters/level", HP_BUS + "/meters/level"]
STEP = float(sys.argv[1]) if len(sys.argv) > 1 else 2
OUT = sys.argv[2] if len(sys.argv) > 2 else "hp_taper.csv"
AMP = 10 ** (-20 / 20)  # -20 dBFS sine
SETTLE, SAMPLE = 0.45, 0.45


def decode_multi(d):
    s, names, parents, leaves, data = (d.get(k, []) for k in ("string", "name", "parent", "path", "data"))
    out = []
    for j, i in enumerate(leaves):
        segs = []
        for _ in range(len(names) + 1):
            segs.append(s[names[i]])
            if parents[i] == i:
                out.append(("/" + "/".join(reversed(segs)), data[j]))
                break
            i = parents[i]
    return out


class Uasrv:
    def __init__(self):
        self.s = socket.create_connection(("127.0.0.1", 4720), timeout=5)
        self.lock = threading.Lock()
        self.replies = {}
        self.samples = []  # (t, path, value)
        threading.Thread(target=self._reader, daemon=True).start()

    def _reader(self):
        buf = b""
        while True:
            try:
                chunk = self.s.recv(1 << 20)
            except OSError:
                return
            if not chunk:
                return
            buf += chunk
            while b"\0" in buf:
                m, buf = buf.split(b"\0", 1)
                o = json.loads(m.decode("utf-8", "replace"))
                if o.get("path") == "multi" and o["data"].get("tag") == "meters":
                    now = time.time()
                    with self.lock:
                        for p, v in decode_multi(o["data"]):
                            self.samples.append((now, p, v))
                else:
                    self.replies[o.get("path")] = o

    def call(self, verb, path, arg=None):
        self.replies.pop(path, None)
        self.s.sendall(f"{verb} {path}{'' if arg is None else ' ' + arg}\0".encode())
        t0 = time.time()
        while path not in self.replies and time.time() - t0 < 5:
            time.sleep(0.005)
        return self.replies.pop(path, None)

    def window(self, t0, t1):
        with self.lock:
            return [(p, v) for t, p, v in self.samples if t0 <= t < t1]


def level_db(values):
    """Meter leaf -> dBFS of the loudest channel (values are linear amplitude per channel)."""
    if isinstance(values, dict):
        values = values.get("value", values)
    nums = [abs(x) for x in (values if isinstance(values, list) else [values]) if isinstance(x, (int, float))]
    peak = max(nums, default=0.0)
    return 20 * math.log10(peak) if peak > 0 else -math.inf


phase = 0.0


def callback(outdata, frames, _time, _status):
    global phase
    t = (np.arange(frames) + phase) / 48000
    outdata[:] = (AMP * np.sin(2 * np.pi * 1000 * t)).astype(np.float32)[:, None]
    phase += frames


srv = Uasrv()
orig = srv.call("get", GAIN)["data"]["properties"]["value"]["value"]
print(f"HP gain at start: {orig}")
for m in METERS:
    print("subscribe", m.split("a7fef601f2c04340a813f379826985d7")[1], srv.call("subscribe", m).get("error", "ok"))

stream = sd.OutputStream(device=None, samplerate=48000, channels=2, dtype="float32", callback=callback)
rows = []
try:
    stream.start()
    time.sleep(1.0)
    # Show the raw meter leaves once so the format is on record.
    for p, v in srv.window(time.time() - 0.2, time.time())[:6]:
        print("  sample", p.split("a7fef601f2c04340a813f379826985d7")[-1], json.dumps(v)[:120])
    v = orig
    values = [round(orig - k * STEP, 3) for k in range(int(orig / STEP) + 1)]
    if values[-1] != 0:
        values.append(0.0)
    for v in values:
        srv.call("set", GAIN + "/value/value", f"{v}")
        time.sleep(SETTLE)
        t0 = time.time()
        time.sleep(SAMPLE)
        win = srv.window(t0, time.time())
        row = {"value": v}
        for p in sorted({p for p, _ in win}):
            dbs = [level_db(x) for q, x in win if q == p]
            finite = [d for d in dbs if math.isfinite(d)]
            row[p.split("a7fef601f2c04340a813f379826985d7")[-1]] = statistics.median(finite) if finite else -math.inf
        rows.append(row)
        print(" ".join(f"{k}={val:.2f}" if isinstance(val, float) else f"{k}={val}" for k, val in row.items()), flush=True)
finally:
    srv.call("set", GAIN + "/value/value", f"{orig}")
    stream.stop()
    stream.close()
    back = srv.call("get", GAIN)["data"]["properties"]["value"]["value"]
    print(f"restored HP gain: {back}, tone stopped")

keys = sorted({k for r in rows for k in r if k != "value"})
with open(OUT, "w", encoding="utf-8") as f:
    f.write("value," + ",".join(keys) + "\n")
    for r in rows:
        f.write(f"{r['value']}," + ",".join(f"{r.get(k, float('nan')):.3f}" for k in keys) + "\n")
print("saved", OUT)
