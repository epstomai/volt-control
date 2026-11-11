import ctypes, subprocess, sys, time
from ctypes import wintypes
u32 = ctypes.windll.user32; u32.SetProcessDPIAware()
h = u32.FindWindowW(None, "Volt Control")
def rect():
    r = wintypes.RECT(); u32.GetWindowRect(h, ctypes.byref(r)); return r.left, r.top, r.right - r.left, r.bottom - r.top
def click(x): subprocess.run([sys.executable, "click.py", str(x), "53"], capture_output=True); time.sleep(0.8)
print("start      ", rect())
u32.ShowWindow(h, 3); time.sleep(0.8); print("maximized  ", rect())
click(260); print("mixer (max)", rect())
u32.ShowWindow(h, 9); time.sleep(0.8); print("restored   ", rect())
click(202); print("main       ", rect())
