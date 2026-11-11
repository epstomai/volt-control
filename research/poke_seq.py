"""Make harmless parameter changes through UASrv and print an epoch timestamp for each,
so hook logs can be aligned. Every change is reverted to the value read at start.

usage: python -I poke_seq.py
"""
import json
import socket
import time

DEV = "/devices/31323334353637383930313233340000"
TONE2 = DEV + "/buses/0dda87cf8ccc4ca7957728c10401ec6a/controls/preamp_tone"
HP = DEV + "/buses/a7fef601f2c04340a813f379826985d7/outputs/main/controls/gain"

s = socket.create_connection(("127.0.0.1", 4720), timeout=3)
buf = b""


def call(line):
    global buf
    s.sendall(line.encode() + b"\0")
    path = line.split(" ")[1]
    while True:
        while b"\0" not in buf:
            buf += s.recv(1 << 20)
        msg, buf = buf.split(b"\0", 1)
        obj = json.loads(msg.decode("utf-8", "replace"))
        if obj.get("path") == path:
            return obj


bright = call(f"get {DEV}")["data"]["properties"]["display_brightness"]["value"]
hp = call(f"get {HP}")["data"]["properties"]["value"]["value"]
tone = call(f"get {TONE2}")["data"]["properties"]["value"]["value"]
print(f"start brightness={bright} hp={hp} tone2={tone}")

steps = [
    ("brightness+1", f"set {DEV}/display_brightness/value {bright + 1}"),
    ("brightness restore", f"set {DEV}/display_brightness/value {bright}"),
    ("tone2 WARMTH", f"set {TONE2}/value/value 1"),
    ("tone2 restore", f"set {TONE2}/value/value {tone}"),
    ("hp -1", f"set {HP}/value/value {hp - 1}"),  # lower first: safe direction
    ("hp restore", f"set {HP}/value/value {hp}"),
]
for label, line in steps:
    print(f"{time.time():.3f} {label}", flush=True)
    call(line)
    time.sleep(1.5)
