"""Simulate a user resize on the main tab, then switch tabs and back; finally put the main size back."""
import ctypes, json, os, subprocess, sys, time
from ctypes import wintypes
u32 = ctypes.windll.user32; u32.SetProcessDPIAware()
h = u32.FindWindowW(None, "Volt Control")
def rect():
    r = wintypes.RECT(); u32.GetWindowRect(h, ctypes.byref(r)); return r.left, r.top, r.right - r.left, r.bottom - r.top
def click(x): subprocess.run([sys.executable, "click.py", str(x), "53"], capture_output=True); time.sleep(0.8)
cfg = os.path.expandvars(r"%APPDATA%\com.knia.voltcontrol\config.json")
sizes = lambda: json.load(open(cfg, encoding="utf-8")).get("window_sizes")
click(202); x, y, w0, h0 = rect(); print("main  ", rect())
u32.SetWindowPos(h, 0, x, y, w0 + 100, h0 + 60, 0x0004 | 0x0010); time.sleep(1.2)
print("after user resize", rect(), "saved:", sizes())
click(260); print("mixer ", rect())
click(202); print("main  ", rect())
u32.SetWindowPos(h, 0, x, y, w0, h0, 0x0004 | 0x0010); time.sleep(1.2)
print("restored main", rect(), "saved:", sizes())
