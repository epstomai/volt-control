"""Does OBS 'AudioMix' (Volt 'MON L/R' capture endpoint) follow the MONITOR volume?
The monitor stays MUTED throughout (speakers are also off); volume and mute are restored."""
import json, math, socket, statistics, sys, threading, time
import numpy as np, sounddevice as sd
import websocket

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
BUS = "/devices/31323334353637383930313233340000/buses/d972b625d5d445249ca716ccf8829db2/outputs/main/controls/"
GAIN, MUTE = BUS + "gain", BUS + "mute"

ws = websocket.create_connection("ws://127.0.0.1:4455", timeout=5); ws.recv()
ws.send(json.dumps({"op": 1, "d": {"rpcVersion": 1, "eventSubscriptions": 1 << 16}}))
meters, lock = [], threading.Lock()


def reader():
    while True:
        m = json.loads(ws.recv())
        if m["op"] == 5 and m["d"]["eventType"] == "InputVolumeMeters":
            with lock:
                for i in m["d"]["eventData"]["inputs"]:
                    if i["inputName"] == "AudioMix" and i["inputLevelsMul"]:
                        meters.append((time.time(), max(ch[0] for ch in i["inputLevelsMul"])))


threading.Thread(target=reader, daemon=True).start()
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


def val(p):
    return call("get", p)["data"]["properties"]["value"]["value"]


def read(label):
    time.sleep(0.5); t0 = time.time(); time.sleep(0.6)
    with lock:
        w = [v for t, v in meters if t >= t0]
    d = statistics.median(20 * math.log10(v) if v > 0 else -math.inf for v in w) if w else -math.inf
    print(f"{label:30s} AudioMix {d:7.1f} dBFS ({len(w)})", flush=True)


ph = [0]


def cb(out, n, *_):
    t = (np.arange(n) + ph[0]) / 48000
    out[:] = (0.1 * np.sin(2 * np.pi * 1000 * t)).astype(np.float32)[:, None]
    ph[0] += n


g0, m0 = val(GAIN), val(MUTE)
print(f"start: monitor volume {g0}, mute {m0}")
st = sd.OutputStream(samplerate=48000, channels=2, dtype="float32", callback=cb)
try:
    call("set", MUTE + "/value/value", "1")
    if val(MUTE) != 1:
        raise SystemExit("could not mute the monitor")
    st.start()
    call("set", GAIN + "/value/value", "0")
    read("tone, muted, monitor vol 0")
    call("set", GAIN + "/value/value", "100")
    read("tone, muted, monitor vol 100")
finally:
    st.stop(); st.close()
    call("set", GAIN + "/value/value", f"{g0}")
    call("set", MUTE + "/value/value", f"{int(m0)}")
    print(f"restored: monitor volume {val(GAIN)}, mute {val(MUTE)}; tone stopped")
