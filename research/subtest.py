import socket, threading, time, json, sys
D = "/devices/31323334353637383930313233340000/buses/"
HP = D + "a7fef601f2c04340a813f379826985d7/outputs/main/controls/gain"
MON = D + "d972b625d5d445249ca716ccf8829db2/outputs/main/controls/gain"
s = socket.create_connection(("127.0.0.1", 4720)); buf = b""; log = []
def rx():
    global buf
    while True:
        d = s.recv(65536)
        if not d: break
        buf += d
        while b"\0" in buf:
            m, buf = buf.split(b"\0", 1); log.append((round(time.time() - t0, 2), m.decode()[:300]))
t0 = time.time(); threading.Thread(target=rx, daemon=True).start()
send = lambda c: s.sendall(c.encode() + b"\0")
send(f"subscribe {HP}"); send(f"subscribe {MON}"); time.sleep(0.5)
c2 = socket.create_connection(("127.0.0.1", 4720))   # second client does the write
c2.sendall(f"set {HP}/value 90".encode() + b"\0"); time.sleep(0.8)
c2.sendall(f"set {HP}/value 100".encode() + b"\0"); time.sleep(0.8)
for l in log: print(*l)
