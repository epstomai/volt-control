import json, socket, sys
G = "/devices/31323334353637383930313233340000/buses/a7fef601f2c04340a813f379826985d7/outputs/main/controls/gain"
s = socket.create_connection(("127.0.0.1", 4720), timeout=3)
s.sendall(f"set {G}/value/value {float(sys.argv[1])}\0".encode()); print(s.recv(4096)[:120])
