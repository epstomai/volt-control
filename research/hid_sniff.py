"""Read-only sniffer for the Volt 2 Gen 2 vendor HID interface (VID 2B5A, PID 002D, MI_04).

Never writes to the device. Logs every input report with a timestamp; identical
consecutive reports are collapsed so periodic status traffic does not drown events.

usage: python -I hid_sniff.py <seconds> <logfile> [hex-prefix-to-skip ...]
e.g. skip the 50 Hz meter frames:  python -I hid_sniff.py 120 keys.log f10c23
"""
import sys
import time

import hid

secs, out_path = float(sys.argv[1]), sys.argv[2]
skip = [bytes.fromhex(p) for p in sys.argv[3:]]
skipped = 0
info = next(d for d in hid.enumerate(0x2B5A, 0x002D) if d["interface_number"] == 4)
dev = hid.device()
dev.open_path(info["path"])
dev.set_nonblocking(False)

t0 = time.time()
last, repeat = None, 0
with open(out_path, "w", encoding="utf-8") as out:
    out.write(f"opened {info['path']!r}\n")
    while time.time() - t0 < secs:
        data = dev.read(1024, 200)  # 200 ms timeout
        if not data:
            continue
        if any(bytes(data[: len(p)]) == p for p in skip):
            skipped += 1
            continue
        line = " ".join(f"{b:02x}" for b in data)
        if line == last:
            repeat += 1
            continue
        if repeat:
            out.write(f"           (previous x{repeat})\n")
            repeat = 0
        out.write(f"{time.time() - t0:9.3f} [{len(data):3d}] {line}\n")
        out.flush()
        last = line
    if repeat:
        out.write(f"           (previous x{repeat})\n")
    out.write(f"skipped {skipped} filtered reports\n")
dev.close()
