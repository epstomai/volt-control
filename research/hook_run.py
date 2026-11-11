"""Attach the HID-write logger to UAMixerHelper for N seconds, then detach cleanly.

usage: python -I hook_run.py <seconds> <logfile> [hook.js]  (lines are stamped with absolute epoch time)
"""
import sys
import time

import frida

secs, out_path = float(sys.argv[1]), sys.argv[2]
script_src = open(sys.argv[3] if len(sys.argv) > 3 else __file__.replace("hook_run.py", "hook_hidwrite.js"), encoding="utf-8").read()
session = frida.attach("UAMixerHelper.exe")
out = open(out_path, "w", encoding="utf-8")
t0 = time.time()


def on_message(msg, _data):
    text = msg.get("payload") if msg["type"] == "send" else f"ERROR {msg.get('description')}"
    out.write(f"{time.time():.3f} {text}\n")
    out.flush()


script = session.create_script(script_src)
script.on("message", on_message)
script.load()
try:
    time.sleep(secs)
finally:
    script.unload()
    session.detach()
    out.close()
