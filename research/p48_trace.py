"""Trace input 1 preamp_48v pushes while the UI turns it OFF, then ON (arm + confirm). Ends ON (as found)."""
import json, socket, subprocess, sys, threading, time
P = "/devices/31323334353637383930313233340000/buses/8aa1d212e4bc4b328aed3c408174d447/controls/preamp_48v"
s = socket.create_connection(("127.0.0.1", 4720)); s.settimeout(0.1); buf = b""; t0 = time.time(); stop = False
def reader():
    global buf
    s.sendall(f"subscribe {P}\0".encode())
    while not stop:
        try: buf += s.recv(1 << 20)
        except socket.timeout: continue
        while b"\0" in buf:
            m, buf = buf.split(b"\0", 1); o = json.loads(m.decode("utf-8", "replace"))
            txt = json.dumps(o, ensure_ascii=False)
            if "preamp_48v" in txt and ("multi" == o.get("path") or o.get("path", "").startswith(P)):
                print(f"{time.time()-t0:6.2f}s push {txt[-160:]}", flush=True)
threading.Thread(target=reader, daemon=True).start()
def ui(label, x=441, y=273):
    print(f"{time.time()-t0:6.2f}s UI click: {label}", flush=True)
    subprocess.run([sys.executable, "click.py", str(x), str(y)], capture_output=True)
def shot(name): subprocess.run([sys.executable, "shot.py", name], capture_output=True)
time.sleep(1)
ui("OFF"); time.sleep(4); shot("p48_off.png")
ui("ON #1 (arm)"); time.sleep(0.5); shot("p48_armed.png")
ui("ON #2 (confirm)")
for i in range(4): time.sleep(1.5); shot(f"p48_on{i}.png")
time.sleep(1); stop = True
