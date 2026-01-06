use bytes::{Buf, BufMut, BytesMut}; // 需要用到 bytes 库处理二进制
use std::io::{self, Cursor};

/// BGP 消息类型 (RFC 4271 Sec 4.1)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BgpMessageType {
    Open = 1,
    Update = 2,
    Notification = 3,
    Keepalive = 4,
}

impl BgpMessageType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Open),
            2 => Some(Self::Update),
            3 => Some(Self::Notification),
            4 => Some(Self::Keepalive),
            _ => None,
        }
    }
}

/// BGP 通用头部 (19 Bytes)
/// 0                   1                   2                   3
/// 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                                                               |
/// +                                                               +
/// |                                                               |
/// +                                                               +
/// |                           Marker                              |
/// +                                                               +
/// |                                                               |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |          Length               |      Type     |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
#[derive(Debug, Clone)]
pub struct BgpHeader {
    pub length: u16,
    pub msg_type: BgpMessageType,
}

impl BgpHeader {
    pub const LENGTH: usize = 19; // 16 Marker + 2 Len + 1 Type

    /// 编码头部到 buffer
    pub fn encode(&self, buf: &mut BytesMut) {
        // 1. 写 Marker (16字节全1)
        for _ in 0..16 {
            buf.put_u8(0xFF);
        }
        // 2. 写 Length
        buf.put_u16(self.length);
        // 3. 写 Type
        buf.put_u8(self.msg_type as u8);
    }

    /// 从 buffer 解码头部
    pub fn decode(buf: &mut Cursor<&[u8]>) -> io::Result<Self> {
        if buf.remaining() < Self::LENGTH {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Header too short"));
        }

        // 1. 跳过 Marker (实际生产环境应该校验它是否全为 0xFF)
        buf.advance(16);

        // 2. 读 Length
        let length = buf.get_u16();
        if length < 19 || length > 4096 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid Length"));
        }

        // 3. 读 Type
        let type_code = buf.get_u8();
        let msg_type = BgpMessageType::from_u8(type_code)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Unknown Message Type"))?;

        Ok(Self { length, msg_type })
    }
}
