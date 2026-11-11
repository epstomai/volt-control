import socket, json, time
DEV = "/devices/31323334353637383930313233340000"
B = DEV + "/buses/"
paths = [B+"8aa1d212e4bc4b328aed3c408174d447/meters/level", B+"d972b625d5d445249ca716ccf8829db2/outputs/main/meters/level", B+"a7fef601f2c04340a813f379826985d7/outputs/main/meters/level", DEV+"/meters"]
s = socket.create_connection(("127.0.0.1", 4720)); s.settimeout(0.3)
for p in paths: s.sendall(f"subscribe {p}\0".encode())
buf = b""; n = 0; t0 = time.time(); kinds = {}
while time.time() - t0 < 3:
    try: buf += s.recv(1 << 20)
    except socket.timeout: continue
    while b"\0" in buf:
        m, buf = buf.split(b"\0", 1); o = json.loads(m); n += 1
        k = o["path"] if o["path"] != "multi" else "multi:" + "/".join(o["data"]["string"][-4:])
        kinds[k] = kinds.get(k, 0) + 1
        if kinds[k] <= 2: print(json.dumps(o)[:350])
print(n, "msgs in 3s", kinds)
print(json.dumps(json.load(open("tree.json", encoding="utf-8"))[DEV + "/meters"])[:800])
