#!/bin/bash
# 必须以 root 权限运行

echo "🔧 正在构建 BGP 测试环境 (Namespace)..."

# 1. 清理旧环境 (防止报错)
ip netns del peer1 2>/dev/null
ip link del veth_root 2>/dev/null

# 2. 创建 Namespace (模拟对端路由器)
ip netns add peer1

# 3. 创建一对虚拟网卡
ip link add veth_root type veth peer name veth_peer

# 4. 把一头插到 peer1 里
ip link set veth_peer netns peer1

# 5. 配置本端 (RustyNOS 所在端) IP
# 我们让 RustyNOS 监听 192.168.10.1
ip addr add 192.168.10.1/24 dev veth_root
ip link set veth_root up

# 6. 配置对端 (模拟邻居) IP
# 模拟邻居 IP 为 192.168.10.2
ip netns exec peer1 ip addr add 192.168.10.2/24 dev veth_peer
ip netns exec peer1 ip link set veth_peer up
ip netns exec peer1 ip link set lo up

echo "✅ 环境搭建完成！"
echo "   - 本机 IP: 192.168.10.1"
echo "   - 邻居 IP: 192.168.10.2 (在 netns 'peer1' 中)"
echo "   - 测试连通性: ping 192.168.10.2"