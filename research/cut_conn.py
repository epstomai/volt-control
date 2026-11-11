"""Reset the TCP connection(s) a given process has to 127.0.0.1:4720 (simulates UASrv kicking it).
Only that client's socket is affected; UAMixerHelper keeps running. Needs admin (SetTcpEntry).
usage: python -I cut_conn.py [process-name]   (default volt-control.exe)"""
import ctypes, socket, struct, sys
import psutil

name = sys.argv[1] if len(sys.argv) > 1 else "volt-control.exe"
pids = {p.pid for p in psutil.process_iter(["name"]) if (p.info["name"] or "").lower() == name.lower()}

class MIB_TCPROW(ctypes.Structure):
    _fields_ = [("dwState", ctypes.c_uint32), ("dwLocalAddr", ctypes.c_uint32), ("dwLocalPort", ctypes.c_uint32),
                ("dwRemoteAddr", ctypes.c_uint32), ("dwRemotePort", ctypes.c_uint32)]

def addr(ip): return struct.unpack("<I", socket.inet_aton(ip))[0]
def port(p): return socket.htons(p)

cut = 0
for c in psutil.net_connections("tcp4"):
    if c.pid in pids and c.raddr and c.raddr.port == 4720 and c.status == "ESTABLISHED":
        row = MIB_TCPROW(12, addr(c.laddr.ip), port(c.laddr.port), addr(c.raddr.ip), port(c.raddr.port))  # 12 = DELETE_TCB
        rc = ctypes.windll.iphlpapi.SetTcpEntry(ctypes.byref(row))
        print(f"cut {c.laddr.ip}:{c.laddr.port} -> {c.raddr.ip}:{c.raddr.port} rc={rc}")
        cut += 1
print(f"{cut} connection(s) cut for pids {sorted(pids)}")
