use bytes::{Buf, BytesMut};
use std::io::{self, Cursor};

/// 属性类型代码 (RFC 4271)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AttributeTypeCode {
    Origin = 1,
    AsPath = 2,
    NextHop = 3,
    MultiExitDisc = 4,
    LocalPref = 5,
    AtomicAggregate = 6,
    Aggregator = 7,
    // 其他暂略
}

impl AttributeTypeCode {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Origin),
            2 => Some(Self::AsPath),
            3 => Some(Self::NextHop),
            4 => Some(Self::MultiExitDisc),
            5 => Some(Self::LocalPref),
            6 => Some(Self::AtomicAggregate),
            7 => Some(Self::Aggregator),
            _ => None,
        }
    }
}

/// 通用属性结构
#[derive(Debug, Clone)]
pub struct Attribute {
    pub flags: u8,
    pub type_code: u8, // 暂时存 u8，方便调试未知属性
    pub value: Vec<u8>,
}

impl Attribute {
    pub fn decode(buf: &mut Cursor<&[u8]>) -> io::Result<Self> {
        if buf.remaining() < 2 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Attribute too short"));
        }

        let flags = buf.get_u8();
        let type_code = buf.get_u8();

        // 检查 Extended Length 标志位 (第4位，0x10)
        let is_extended = (flags & 0x10) != 0;

        let length = if is_extended {
             if buf.remaining() < 2 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Ext attr len missing"));
             }
             buf.get_u16() as usize
        } else {
             if buf.remaining() < 1 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Attr len missing"));
             }
             buf.get_u8() as usize
        };

        if buf.remaining() < length {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Attribute value truncated"));
        }

        // 读取属性值 (暂时不解析具体内容，先存 Raw Bytes)
        let mut value = vec![0u8; length];
        buf.copy_to_slice(&mut value);

        Ok(Self {
            flags,
            type_code,
            value,
        })
    }
}