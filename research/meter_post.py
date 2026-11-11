"""Is the MONITOR output meter post-volume? Monitor stays MUTED the whole time (speakers silent).
Plays a -20 dBFS 1 kHz tone, reads the master MONITOR output meter at the current volume and at
half of it (only lowering), then restores volume and mute exactly."""
import json, math, socket, time
import numpy as np, sounddevice as sd

B = "/devices/31323334353637383930313233340000/buses/"
MB = B + "b0dc87cf068741b5933ebb676712dc6b"
GAIN, MUTE, METER = MB + "/outputs/main/controls/gain", MB + "/outputs/main/controls/mute", MB + "/outputs/main/meters/level"

s = socket.create_connection(("127.0.0.1", 4720)); s.settimeout(0.05); buf = b""; replies = {}


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
                d = o["data"]
                for j, i in enumerate(d["path"]):
                    if sink is not None and d["string"][d["name"][i]] == "peak":
                        sink.append(max(d["data"][j]))
            else:
                replies[o.get("path")] = o


def call(v, p, a=None):
    replies.pop(p, None)
    s.sendall(f"{v} {p}{'' if a is None else ' ' + a}\0".encode())
    for _ in range(200):
        pump(0.02)
        if p in replies:
            return replies.pop(p)


def val(p):
    return call("get", p)["data"]["properties"]["value"]["value"]


def read(label):
    pump(0.5)
    w = []
    pump(0.8, w)
    db = 20 * math.log10(max(w)) if w and max(w) > 0 else -math.inf
    print(f"{label:34s} peak {db:7.1f} dBFS ({len(w)} pushes)", flush=True)


g0, m0 = val(GAIN), val(MUTE)
print(f"start: monitor volume {g0}, mute {m0}")
if not m0:
    raise SystemExit("monitor is not muted; refusing to play a tone through the speakers")
call("subscribe", METER)
ph = [0]


def cb(out, n, *_):
    t = (np.arange(n) + ph[0]) / 48000
    out[:] = (0.1 * np.sin(2 * np.pi * 1000 * t)).astype(np.float32)[:, None]
    ph[0] += n


st = sd.OutputStream(samplerate=48000, channels=2, dtype="float32", callback=cb)
try:
    read("no tone")
    st.start()
    read(f"tone, muted, volume {g0}")
    call("set", GAIN + "/value/value", f"{round(g0 / 2, 1)}")
    read(f"tone, muted, volume {round(g0 / 2, 1)}")
    call("set", GAIN + "/value/value", "0")
    read("tone, muted, volume 0")
finally:
    st.stop(); st.close()
    call("set", GAIN + "/value/value", f"{g0}")
    call("set", MUTE + "/value/value", f"{int(m0)}")
    print(f"restored: monitor volume {val(GAIN)}, mute {val(MUTE)}")
