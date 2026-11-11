"""Resize the Volt Control window to W x H (window size incl. frame)."""
import ctypes, sys
from ctypes import wintypes
u32 = ctypes.windll.user32; u32.SetProcessDPIAware()
h = u32.FindWindowW(None, "Volt Control")
r = wintypes.RECT(); u32.GetWindowRect(h, ctypes.byref(r))
u32.MoveWindow(h, r.left, r.top, int(sys.argv[1]), int(sys.argv[2]), True)
u32.GetWindowRect(h, ctypes.byref(r)); print("now", r.right - r.left, r.bottom - r.top)
