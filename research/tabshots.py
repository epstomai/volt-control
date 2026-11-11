"""Click each Volt Control tab (positions read from the live DOM are not available, so use x offsets) and screenshot."""
import ctypes, subprocess, sys, time
from ctypes import wintypes
u32 = ctypes.windll.user32; u32.SetProcessDPIAware()
def rect():
    h = u32.FindWindowW(None, "Volt Control"); r = wintypes.RECT(); u32.GetWindowRect(h, ctypes.byref(r)); return r.left, r.top, r.right - r.left, r.bottom - r.top
for name, x in [(a.split(":")[0], int(a.split(":")[1])) for a in sys.argv[1:]]:
    subprocess.run([sys.executable, "click.py", str(x), "53"], capture_output=True)
    time.sleep(0.8)
    subprocess.run([sys.executable, "shot.py", f"tab_{name}.png"], capture_output=True)
    print(name, "window", rect())
