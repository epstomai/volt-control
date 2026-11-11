"""Read audio levels from OBS (obs-websocket v5) while sweeping the Volt HEADPHONE volume.

Plays a -20 dBFS 1 kHz tone to the default output (Volt MON L/R). Mode `probe` compares every OBS
audio input at HP 100 vs 50; mode `sweep <input>` steps HP 100 -> 0 and records that input.
The HP volume only goes down from its current value and is always restored; the tone is stopped.
usage: python -I obs_meter.py probe
       python -I obs_meter.py sweep "<OBS input name>" [step] [out.csv]
"""
import json, math, socket, statistics, sys, threading, time
import numpy as np, sounddevice as sd
import websocket

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

HP_GAIN = "/devices/31323334353637383930313233340000/buses/a7fef601f2c04340a813f379826985d7/outputs/main/controls/gain"


class Obs:
    def __init__(self, url="ws://127.0.0.1:4455"):
        self.ws = websocket.create_connection(url, timeout=5)
        hello = json.loads(self.ws.recv())
        assert hello["op"] == 0, hello
        self.ws.send(json.dumps({"op": 1, "d": {"rpcVersion": 1, "eventSubscriptions": 1 << 16}}))  # InputVolumeMeters
        while json.loads(self.ws.recv())["op"] != 2:
            pass
        self.lock = threading.Lock()
        self.meters = []  # (t, name, [[mag, peak, inputPeak] per channel])
        self.replies = {}
        self.n = 0
        threading.Thread(target=self._reader, daemon=True).start()

    def _reader(self):
        while True:
            try:
                msg = json.loads(self.ws.recv())
            except Exception:
                return
            if msg["op"] == 5 and msg["d"]["eventType"] == "InputVolumeMeters":
                now = time.time()
                with self.lock:
                    for i in msg["d"]["eventData"]["inputs"]:
                        self.meters.append((now, i["inputName"], i["inputLevelsMul"]))
            elif msg["op"] == 7:
                self.replies[msg["d"]["requestId"]] = msg["d"]

    def request(self, kind, data=None):
        self.n += 1
        rid = str(self.n)
        self.ws.send(json.dumps({"op": 6, "d": {"requestType": kind, "requestId": rid, "requestData": data or {}}}))
        for _ in range(500):
            if rid in self.replies:
                return self.replies.pop(rid).get("responseData", {})
            time.sleep(0.01)
        raise RuntimeError(f"OBS: no reply to {kind}")

    def levels(self, t0, t1):
        """input name -> median dBFS of the loudest channel's magnitude (OBS 'mul' is linear)."""
        with self.lock:
            win = [(n, l) for t, n, l in self.meters if t0 <= t < t1]
        out = {}
        for name in {n for n, _ in win}:
            vals = [max((ch[0] for ch in l), default=0.0) for n, l in win if n == name]
            dbs = [20 * math.log10(v) if v > 0 else -math.inf for v in vals]
            out[name] = statistics.median(dbs) if dbs else -math.inf
        return out


class Uasrv:
    def __init__(self):
        self.s = socket.create_connection(("127.0.0.1", 4720), timeout=5)
        self.buf = b""

    def call(self, verb, path, arg=None):
        self.s.sendall(f"{verb} {path}{'' if arg is None else ' ' + arg}\0".encode())
        while True:
            while b"\0" not in self.buf:
                self.buf += self.s.recv(1 << 20)
            m, self.buf = self.buf.split(b"\0", 1)
            o = json.loads(m.decode("utf-8", "replace"))
            if o.get("path") == path:
                return o

    def hp(self, v=None):
        if v is not None:
            self.call("set", HP_GAIN + "/value/value", f"{v}")
        return self.call("get", HP_GAIN)["data"]["properties"]["value"]["value"]


ph = [0]


def tone(out, n, *_):
    t = (np.arange(n) + ph[0]) / 48000
    out[:] = (0.1 * np.sin(2 * np.pi * 1000 * t)).astype(np.float32)[:, None]
    ph[0] += n


def measure(obs, settle=0.5, window=0.6):
    time.sleep(settle)
    t0 = time.time()
    time.sleep(window)
    return obs.levels(t0, time.time())


obs, ua = Obs(), Uasrv()
mode = sys.argv[1] if len(sys.argv) > 1 else "probe"
if mode == "probe":
    for i in obs.request("GetInputList")["inputs"]:
        print("input:", i["inputName"], "|", i["inputKind"])
hp0 = ua.hp()
print("HP volume at start:", hp0)
stream = sd.OutputStream(samplerate=48000, channels=2, dtype="float32", callback=tone)
rows = []
try:
    print("no tone      :", {k: round(v, 1) for k, v in measure(obs).items()})
    stream.start()
    if mode == "probe":
        print(f"tone, HP {hp0}:", {k: round(v, 1) for k, v in measure(obs).items()})
        ua.hp(round(hp0 / 2, 1))
        print(f"tone, HP {round(hp0 / 2, 1)} :", {k: round(v, 1) for k, v in measure(obs).items()})
    else:
        name = sys.argv[2]
        step = float(sys.argv[3]) if len(sys.argv) > 3 else 2
        v = hp0
        while v >= 0:
            ua.hp(round(v, 3))
            lv = measure(obs).get(name, -math.inf)
            rows.append((round(v, 3), lv))
            print(f"HP {v:6.1f}  {name}: {lv:7.2f} dBFS", flush=True)
            v -= step
finally:
    stream.stop()
    stream.close()
    print("restored HP volume:", ua.hp(hp0), "; tone stopped")

if rows:
    out = sys.argv[4] if len(sys.argv) > 4 else "hp_taper_obs.csv"
    with open(out, "w", encoding="utf-8") as f:
        f.write("hp_volume,level_dbfs\n")
        for v, lv in rows:
            f.write(f"{v},{lv:.3f}\n")
    print("saved", out)
