import ctypes, subprocess, sys, time
from ctypes import wintypes
u32 = ctypes.windll.user32; u32.SetProcessDPIAware()
h = u32.FindWindowW(None, "Volt Control")
def rect():
    r = wintypes.RECT(); u32.GetWindowRect(h, ctypes.byref(r)); return r.left, r.top, r.right - r.left, r.bottom - r.top
def run(*a): subprocess.run([sys.executable, *a], capture_output=True)
run("click.py", "320", "53"); time.sleep(0.8); print("midi", rect())
run("click.py", "57", "265")  # 应用 -> toast "MIDI 端口已更新"
for i, d in enumerate([0.03, 0.25, 4.5]):
    time.sleep(d); run("shot.py", f"toast{i}.png")
# narrow MIDI layout, then put the size back
x, y, w, hh = rect()
u32.SetWindowPos(h, 0, x, y, 720, hh + 200, 0x0004 | 0x0010); time.sleep(1.2); run("shot.py", "midi_narrow.png"); print("narrow", rect())
u32.SetWindowPos(h, 0, x, y, w, hh, 0x0004 | 0x0010); time.sleep(1.2); print("back", rect())
run("click.py", "202", "53"); time.sleep(0.8); print("main", rect())
