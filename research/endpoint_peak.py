"""Read-only: peak meter of every active render endpoint and of each audio session on it (pycaw)."""
import math, sys, time
from ctypes import POINTER, cast
from comtypes import CLSCTX_ALL, CoInitialize
from pycaw.pycaw import AudioUtilities, IAudioMeterInformation
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
CoInitialize()
db = lambda v: f"{20*math.log10(v):6.1f} dBFS" if v > 0 else "  -inf"
for dev in AudioUtilities.GetAllDevices():
    if dev.state.value != 1 or "Volt" not in (dev.FriendlyName or ""): continue
    try:
        m = cast(dev._dev.Activate(IAudioMeterInformation._iid_, CLSCTX_ALL, None), POINTER(IAudioMeterInformation))
        pk = max(m.GetPeakValue() for _ in range(20) if time.sleep(0.05) is None)
        print(f"{dev.FriendlyName:45s} {db(pk)}")
    except Exception as e:
        pass
for s in AudioUtilities.GetAllSessions():
    try:
        m = s._ctl.QueryInterface(IAudioMeterInformation); pk = m.GetPeakValue()
        if pk > 0: print("  session", s.Process.name() if s.Process else s.DisplayName, db(pk))
    except Exception:
        pass
