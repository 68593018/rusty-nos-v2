
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

# 构造一个简单的 UPDATE 包: 通告 10.10.10.0/24
def create_update_msg():
    # --- 1. Withdrawn Routes (空) ---
    # Unfeasible Routes Len (2 bytes) = 0
    withdrawn_len = struct.pack("!H", 0)
    
    # --- 2. Path Attributes ---
    # 我们需要构造最基本的属性: ORIGIN, AS_PATH, NEXT_HOP
    
    # Attr 1: ORIGIN (Type=1, Flags=0x40(Transitive), Len=1, Value=0(IGP))
    attr_origin = b'\x40\x01\x01\x00'
    
    # Attr 2: AS_PATH (Type=2, Flags=0x40, Len=4)
    # Value: SegType=2(Seq), SegLen=1, AS=65001
    attr_aspath = b'\x40\x02\x04\x02\x01\xfd\xe9' # 0xfde9 = 65001
    
    # Attr 3: NEXT_HOP (Type=3, Flags=0x40, Len=4, IP=1.1.1.1)
    attr_nexthop = b'\x40\x03\x04' + socket.inet_aton("1.1.1.1")
    
    all_attrs = attr_origin + attr_aspath + attr_nexthop
    attr_len = struct.pack("!H", len(all_attrs))
    
    # --- 3. NLRI (10.10.10.0/24) ---
    # Format: Len(1 byte) + Prefix(variable)
    # /24 = 24 (0x18), Prefix = 10.10.10 (3 bytes)
    nlri = b'\x18\x0a\x0a\x0a' # 10.10.10.0/24
    
    # --- 组装 Body ---
    body = withdrawn_len + attr_len + all_attrs + nlri
    
    # --- 组装 Header ---
    marker = b'\xff' * 16
    length = 19 + len(body)
    msg_type = 2 # UPDATE
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

    time.sleep(1) # 等一秒
    
    # 发送 Keepalive (Header only, type 4)
    ka_msg = b'\xff'*16 + struct.pack("!HB", 19, 4)
    s.sendall(ka_msg)
    print("📤 发送 Keepalive")
    
    time.sleep(1)
    
    # 发送 UPDATE
    update_msg = create_update_msg()
    s.sendall(update_msg)
    print("📤 发送 UPDATE (10.10.10.0/24)")

    # 保持连接
    while True:
        time.sleep(1)

if __name__ == "__main__":
    run()

