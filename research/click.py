"""Post a left click to the Volt Control webview at window-relative (x, y) without moving the real cursor."""
import ctypes, sys, time
from ctypes import wintypes
u32 = ctypes.windll.user32; u32.SetProcessDPIAware()
top = u32.FindWindowW(None, "Volt Control")
x, y = int(sys.argv[1]), int(sys.argv[2])
phase = sys.argv[3] if len(sys.argv) > 3 else "click"  # click | down | up
found = []
EnumProc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
def cb(h, _):
    name = ctypes.create_unicode_buffer(256); u32.GetClassNameW(h, name, 256)
    if name.value == "Chrome_RenderWidgetHostHWND": found.append(h)
    return True
u32.EnumChildWindows(top, EnumProc(cb), 0)
target = found[0]
wr, cr = wintypes.RECT(), wintypes.RECT()
u32.GetWindowRect(top, ctypes.byref(wr)); u32.GetWindowRect(target, ctypes.byref(cr))
cx, cy = x - (cr.left - wr.left), y - (cr.top - wr.top)
lp = (cy << 16) | (cx & 0xFFFF)
if phase in ("click", "down"):
    u32.PostMessageW(target, 0x0200, 0, lp); time.sleep(0.05)   # WM_MOUSEMOVE
    u32.PostMessageW(target, 0x0201, 1, lp); time.sleep(0.05)   # WM_LBUTTONDOWN
if phase in ("click", "up"):
    u32.PostMessageW(target, 0x0202, 0, lp)                     # WM_LBUTTONUP
print("clicked", cx, cy)
