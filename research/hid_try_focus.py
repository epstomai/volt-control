"""Send candidate focus frames to the Volt vendor HID interface and log the device's replies.

Only sends the exact frames the device itself emits when the HP key is pressed:
  f0 12 01 00 f7  (device reports: switched to monitor speakers)
  f0 12 00 00 f7  (device reports: switched to headphone)
Frames are zero-padded to the 39-byte output report length seen from UAMixerHelper.

usage: python -I hid_try_focus.py [hex-frame ...]   (default: the two frames above)
"""
import sys
import time

import hid

REPORT_LEN = 39
frames = sys.argv[1:] or ["f0120100f7", "f0120000f7"]

info = next(d for d in hid.enumerate(0x2B5A, 0x002D) if d["interface_number"] == 4)
dev = hid.device()
dev.open_path(info["path"])


def listen(secs):
    t0 = time.time()
    while time.time() - t0 < secs:
        data = dev.read(1024, 100)
        if data and not bytes(data[:3]) == b"\xf1\x0c\x23":
            print(f"   <- {' '.join(f'{b:02x}' for b in data[:16])}", flush=True)


for frame in frames:
    buf = bytes.fromhex(frame).ljust(REPORT_LEN, b"\0")
    n = dev.write(buf)
    print(f"{time.time():.3f} -> {frame} (write returned {n})", flush=True)
    listen(3.0)
dev.close()
