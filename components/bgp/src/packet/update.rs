use bytes::Buf;
use std::io::{self, Cursor};
use std::net::Ipv4Addr;
use ipnet::Ipv4Net; // 需要 ipnet 库来存储前缀
use super::attribute::Attribute;

#[derive(Debug, Clone)]
pub struct UpdateMessage {
    pub withdrawn_routes: Vec<Ipv4Net>,
    pub attributes: Vec<Attribute>,
    pub nlri: Vec<Ipv4Net>, // Network Layer Reachability Information (新增路由)
}

impl UpdateMessage {
    pub fn decode(buf: &mut Cursor<&[u8]>) -> io::Result<Self> {
        // 1. Unfeasible Routes Length (撤销路由长度)
        if buf.remaining() < 2 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Update header too short"));
        }
        let withdrawn_len = buf.get_u16() as usize;

        // 2. Parse Withdrawn Routes
        let withdrawn_end = buf.position() as usize + withdrawn_len;
        let mut withdrawn_routes = Vec::new();
        while (buf.position() as usize) < withdrawn_end {
            let prefix = decode_prefix(buf)?;
            withdrawn_routes.push(prefix);
        }

        // 3. Total Path Attribute Length
        if buf.remaining() < 2 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Attr len missing"));
        }
        let attr_len = buf.get_u16() as usize;

        // 4. Parse Attributes
        let attr_end = buf.position() as usize + attr_len;
        let mut attributes = Vec::new();
        while (buf.position() as usize) < attr_end {
            let attr = Attribute::decode(buf)?;
            attributes.push(attr);
        }

        // 5. Parse NLRI (剩余所有数据都是新增路由)
        let mut nlri = Vec::new();
        while buf.remaining() > 0 {
            let prefix = decode_prefix(buf)?;
            nlri.push(prefix);
        }

        Ok(Self {
            withdrawn_routes,
            attributes,
            nlri,
        })
    }
}

/// 辅助函数：解析 BGP 格式的前缀 (Length-Value)
/// 格式: [Prefix Len (1 byte)] + [Prefix Bytes (variable)]
fn decode_prefix(buf: &mut Cursor<&[u8]>) -> io::Result<Ipv4Net> {
    if buf.remaining() < 1 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Prefix len missing"));
    }
    let prefix_len = buf.get_u8(); // e.g., 24
    
    // 计算需要的字节数: ceil(len / 8)
    // /24 -> 3 bytes, /25 -> 4 bytes
    let num_bytes = ((prefix_len + 7) / 8) as usize;

    if buf.remaining() < num_bytes {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Prefix data truncated"));
    }

    // 读取压缩的 IP 字节
    let mut ip_bytes = [0u8; 4]; // IPv4 默认全0
    for i in 0..num_bytes {
        ip_bytes[i] = buf.get_u8();
    }
    
    let ip = Ipv4Addr::from(ip_bytes);
    
    // 构造 Ipv4Net
    Ipv4Net::new(ip, prefix_len)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}