
import socket
import struct
import time

# 模拟邻居配置
PEER_IP = "192.168.10.2"
TARGET_IP = "192.168.10.1"
TARGET_PORT = 179

def create_open_msg(my_as, router_id, hold_time=180):
    # 1. Open Body
    # Version(1) + AS(2) + HoldTime(2) + BGPID(4) + OptLen(1)
    # RouterID needs to be packed to 4 bytes
    bgp_id = socket.inet_aton(router_id)
    body = struct.pack("!BHH4sB", 4, my_as, hold_time, bgp_id, 0)

    # 2. Header
    # Marker(16) + Length(2) + Type(1)
    marker = b'\xff' * 16
    length = 19 + len(body)
    msg_type = 1 # OPEN
    header = struct.pack("!16sHB", marker, length, msg_type)

    return header + body

def run():
    print(f"🤖 模拟 BGP 邻居 ({PEER_IP}) 启动...")
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.connect((TARGET_IP, TARGET_PORT))
    print(f"✅ 连接到 RustyNOS ({TARGET_IP})")

    # 1. 接收 RustyNOS 发来的 OPEN
    data = s.recv(1024)
    print(f"📥 收到 RustyNOS 数据 ({len(data)} bytes): {data.hex()}")

    # 2. 发送我们的 OPEN
    open_packet = create_open_msg(my_as=64513, router_id="2.2.2.2")
    s.sendall(open_packet)
    print(f"📤 发送 OPEN 报文")

    # 保持连接
    while True:
        time.sleep(1)

if __name__ == "__main__":
    run()

