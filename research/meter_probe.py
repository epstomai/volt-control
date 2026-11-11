"""Is the only pushing meter (MONITOR d972 output) post-mute / post-HP-volume?
Mutes the monitor first (speakers silent), plays a -20 dBFS tone, reads the meter at HP 100 and 50,
then restores the monitor mute state and HP volume."""
import json, math, socket, threading, time
import numpy as np, sounddevice as sd
DEV = "/devices/31323334353637383930313233340000"
B = DEV + "/buses/"
MON = B + "b0dc87cf068741b5933ebb676712dc6b/outputs/main/controls/mute"
HP = B + "a7fef601f2c04340a813f379826985d7/outputs/main/controls/gain"
METER = B + "d972b625d5d445249ca716ccf8829db2/outputs/main/meters/level"
s = socket.create_connection(("127.0.0.1", 4720)); replies = {}; peaks = []; lock = threading.Lock()
def reader():
    buf = b""
    while True:
        c = s.recv(1 << 20)
        if not c: return
        buf += c
        while b"\0" in buf:
            m, buf = buf.split(b"\0", 1); o = json.loads(m.decode("utf-8", "replace"))
            if o.get("path") == "multi":
                d = o["data"]; st = d["string"]
                for j, i in enumerate(d["path"]):
                    if st[d["name"][i]] == "peak":
                        k = i
                        while d["parent"][k] != k and st[d["name"][k]] != "buses": k = d["parent"][k]
                        bus = [st[d["name"][x]] for x in range(len(d["name"])) if d["parent"][x] == k][:1]
                        with lock: peaks.append((time.time(), max(d["data"][j]), tuple(bus)))
            else: replies[o.get("path")] = o
threading.Thread(target=reader, daemon=True).start()
def call(v, p, a=None):
    replies.pop(p, None); s.sendall(f"{v} {p}{'' if a is None else ' ' + a}\0".encode())
    for _ in range(500):
        if p in replies: return replies.pop(p)
        time.sleep(0.01)
def val(p): return call("get", p)["data"]["properties"]["value"]["value"]
def read(label):
    time.sleep(0.5); t0 = time.time(); time.sleep(0.6)
    with lock: w = [(v, b) for t, v, b in peaks if t >= t0]
    for bus in sorted({b for _, b in w}):
        vs = [v for v, b in w if b == bus]
        db = 20 * math.log10(max(vs)) if max(vs) > 0 else -math.inf
        print(f"{label:28s} {str(bus)[:30]:30s} peak {db:7.1f} dBFS ({len(vs)} pushes)", flush=True)
    if not w: print(f"{label:28s} no meter pushes", flush=True)
mute0, hp0 = val(MON), val(HP)
print("start: monitor mute", mute0, "HP", hp0)
ph = [0]
def cb(out, n, *_):
    t = (np.arange(n) + ph[0]) / 48000; out[:] = (0.1 * np.sin(2 * np.pi * 1000 * t)).astype(np.float32)[:, None]; ph[0] += n
call("set", MON + "/value/value", "1")
call("subscribe", METER)
call("subscribe", B + "a7fef601f2c04340a813f379826985d7/outputs/main/meters/level")
call("subscribe", DEV + "/meters")
st = sd.OutputStream(samplerate=48000, channels=2, dtype="float32", callback=cb)
try:
    read("no tone (muted)")
    st.start()
    read("tone, mon muted, HP 100")
    call("set", HP + "/value/value", "50")
    read("tone, mon muted, HP 50")
finally:
    st.stop(); st.close()
    call("set", HP + "/value/value", f"{hp0}")
    call("set", MON + "/value/value", f"{int(mute0)}")
    print("restored: monitor mute", val(MON), "HP", val(HP))
