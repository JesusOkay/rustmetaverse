//! LLSD binary serialization and deserialization.
//!
//! Binary LLSD is a compact format where each value is prefixed by a single
//! type tag byte. Integers, reals, sizes, and UUIDs are big-endian; dates are
//! little-endian doubles (seconds since epoch).
//!
//! See <https://wiki.secondlife.com/wiki/LLSD#Binary_Serialization>

use crate::osd::OSD;
use chrono::{DateTime, TimeZone, Utc};
use indexmap::IndexMap;
use rustmetaverse_types::UUID;
use std::io::{Cursor, Read, Write};

/// Binary LLSD header prefix.
pub const BINARY_HEADER: &[u8] = b"<?llsd/binary?>\n";

/// Serialize an [`OSD`] value to binary LLSD bytes (with header).
pub fn format_binary(value: &OSD) -> Vec<u8> {
    let mut buf = Vec::with_capacity(256);
    buf.write_all(BINARY_HEADER).unwrap();
    write_binary(&mut buf, value);
    buf
}

/// Serialize an [`OSD`] value to a writer (no header).
pub fn write_binary<W: Write>(stream: &mut W, value: &OSD) {
    match value {
        OSD::Undef => {
            let _ = stream.write_all(b"!");
        }
        OSD::Boolean(true) => {
            let _ = stream.write_all(b"1");
        }
        OSD::Boolean(false) => {
            let _ = stream.write_all(b"0");
        }
        OSD::Integer(v) => {
            let _ = stream.write_all(b"i");
            let _ = stream.write_all(&v.to_be_bytes());
        }
        OSD::Real(v) => {
            let _ = stream.write_all(b"r");
            let _ = stream.write_all(&v.to_be_bytes());
        }
        OSD::UUID(v) => {
            let _ = stream.write_all(b"u");
            let _ = stream.write_all(v.as_bytes());
        }
        OSD::String(s) => {
            let bytes = s.as_bytes();
            let _ = stream.write_all(b"s");
            let _ = stream.write_all(&(bytes.len() as i32).to_be_bytes());
            let _ = stream.write_all(bytes);
        }
        OSD::Uri(s) => {
            let bytes = s.as_bytes();
            let _ = stream.write_all(b"l");
            let _ = stream.write_all(&(bytes.len() as i32).to_be_bytes());
            let _ = stream.write_all(bytes);
        }
        OSD::Date(d) => {
            let _ = stream.write_all(b"d");
            let secs = d.timestamp_micros() as f64 / 1e6;
            let _ = stream.write_all(&secs.to_le_bytes());
        }
        OSD::Binary(b) => {
            let _ = stream.write_all(b"b");
            let _ = stream.write_all(&(b.len() as i32).to_be_bytes());
            let _ = stream.write_all(b);
        }
        OSD::Array(arr) => {
            let _ = stream.write_all(b"[");
            let _ = stream.write_all(&(arr.len() as i32).to_be_bytes());
            for item in arr {
                write_binary(stream, item);
            }
            let _ = stream.write_all(b"]");
        }
        OSD::Map(m) => {
            let _ = stream.write_all(b"{");
            let _ = stream.write_all(&(m.len() as i32).to_be_bytes());
            for (k, v) in m {
                let kbytes = k.as_bytes();
                let _ = stream.write_all(b"k");
                let _ = stream.write_all(&(kbytes.len() as i32).to_be_bytes());
                let _ = stream.write_all(kbytes);
                write_binary(stream, v);
            }
            let _ = stream.write_all(b"}");
        }
    }
}

/// Parse binary LLSD from bytes (with or without header).
pub fn parse_binary(data: &[u8]) -> Result<OSD, String> {
    // Strip header if present
    let data = if data.starts_with(BINARY_HEADER) {
        &data[BINARY_HEADER.len()..]
    } else {
        data
    };
    let cursor = Cursor::new(data);
    let mut parser = BinaryParser { cursor };
    parser.parse_value()
}

struct BinaryParser<'a> {
    cursor: Cursor<&'a [u8]>,
}

impl<'a> BinaryParser<'a> {
    fn read_byte(&mut self) -> Result<u8, String> {
        let mut buf = [0u8; 1];
        self.cursor
            .read_exact(&mut buf)
            .map_err(|_| "Unexpected end of binary LLSD".to_string())?;
        Ok(buf[0])
    }

    fn read_bytes(&mut self, n: usize) -> Result<Vec<u8>, String> {
        let mut buf = vec![0u8; n];
        self.cursor
            .read_exact(&mut buf)
            .map_err(|_| format!("Unexpected end of binary LLSD: needed {} bytes", n))?;
        Ok(buf)
    }

    fn read_i32(&mut self) -> Result<i32, String> {
        let bytes = self.read_bytes(4)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_f64_be(&mut self) -> Result<f64, String> {
        let bytes = self.read_bytes(8)?;
        Ok(f64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_f64_le(&mut self) -> Result<f64, String> {
        let bytes = self.read_bytes(8)?;
        Ok(f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_string(&mut self) -> Result<String, String> {
        let size = self.read_i32()? as usize;
        let bytes = self.read_bytes(size)?;
        String::from_utf8(bytes).map_err(|_| "Invalid UTF-8 in LLSD string".to_string())
    }

    fn parse_value(&mut self) -> Result<OSD, String> {
        let tag = self.read_byte()?;
        match tag {
            b'!' => Ok(OSD::Undef),
            b'1' => Ok(OSD::Boolean(true)),
            b'0' => Ok(OSD::Boolean(false)),
            b'i' => {
                let v = self.read_i32()?;
                Ok(OSD::Integer(v))
            }
            b'r' => {
                let v = self.read_f64_be()?;
                Ok(OSD::Real(v))
            }
            b'u' => {
                let bytes = self.read_bytes(16)?;
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&bytes);
                Ok(OSD::UUID(UUID::from_bytes(arr)))
            }
            b's' => {
                let s = self.read_string()?;
                Ok(OSD::String(s))
            }
            b'l' => {
                let s = self.read_string()?;
                Ok(OSD::Uri(s))
            }
            b'd' => {
                let secs = self.read_f64_le()?;
                let dt = Utc
                    .timestamp_opt(secs as i64, 0)
                    .single()
                    .unwrap_or(DateTime::<Utc>::MIN_UTC);
                Ok(OSD::Date(dt))
            }
            b'b' => {
                let size = self.read_i32()? as usize;
                let bytes = self.read_bytes(size)?;
                Ok(OSD::Binary(bytes))
            }
            b'[' => self.parse_array(),
            b'{' => self.parse_map(),
            _ => Err(format!("Invalid binary LLSD tag: 0x{:02x}", tag)),
        }
    }

    fn parse_array(&mut self) -> Result<OSD, String> {
        let count = self.read_i32()? as usize;
        let mut arr = Vec::with_capacity(count);
        for _ in 0..count {
            arr.push(self.parse_value()?);
        }
        // Expect closing ']'
        let close = self.read_byte()?;
        if close != b']' {
            return Err("Invalid array close token in binary LLSD".to_string());
        }
        Ok(OSD::Array(arr))
    }

    fn parse_map(&mut self) -> Result<OSD, String> {
        let count = self.read_i32()? as usize;
        let mut map = IndexMap::with_capacity(count);
        for _ in 0..count {
            let key_tag = self.read_byte()?;
            let key = match key_tag {
                b'k' => self.read_string()?,
                _ => return Err(format!("Invalid map key tag: 0x{:02x}", key_tag)),
            };
            let value = self.parse_value()?;
            map.insert(key, value);
        }
        // Expect closing '}'
        let close = self.read_byte()?;
        if close != b'}' {
            return Err("Invalid map close token in binary LLSD".to_string());
        }
        Ok(OSD::Map(map))
    }
}

impl OSD {
    /// Serialize to binary LLSD bytes (with header).
    pub fn to_binary(&self) -> Vec<u8> {
        format_binary(self)
    }

    /// Parse binary LLSD from bytes.
    pub fn from_binary(data: &[u8]) -> Result<Self, String> {
        parse_binary(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_binary_scalars() {
        let values = vec![
            OSD::Undef,
            OSD::Boolean(true),
            OSD::Boolean(false),
            OSD::Integer(42),
            OSD::Integer(-1),
            OSD::Integer(0),
            OSD::Real(42.195),
            OSD::Real(0.0),
            OSD::String("hello".to_string()),
            OSD::String("".to_string()),
            OSD::String("üñïçödé".to_string()),
            OSD::Binary(vec![0xDE, 0xAD, 0xBE, 0xEF]),
            OSD::Binary(vec![]),
            OSD::Uri("https://example.com".to_string()),
        ];
        for v in values {
            let bytes = v.to_binary();
            let parsed = OSD::from_binary(&bytes).unwrap();
            assert_eq!(v, parsed, "roundtrip failed for {:?}", v);
        }
    }

    #[test]
    fn roundtrip_binary_uuid() {
        let v = OSD::UUID(UUID::parse("12345678-1234-1234-1234-123456789abc").unwrap());
        let bytes = v.to_binary();
        let parsed = OSD::from_binary(&bytes).unwrap();
        assert_eq!(v, parsed);
    }

    #[test]
    fn roundtrip_binary_array() {
        let v = OSD::Array(vec![
            OSD::Integer(1),
            OSD::String("two".to_string()),
            OSD::Boolean(true),
        ]);
        let bytes = v.to_binary();
        let parsed = OSD::from_binary(&bytes).unwrap();
        assert_eq!(v, parsed);
    }

    #[test]
    fn roundtrip_binary_map() {
        let mut m = IndexMap::new();
        m.insert("name".to_string(), OSD::String("test".to_string()));
        m.insert("count".to_string(), OSD::Integer(7));
        m.insert("active".to_string(), OSD::Boolean(true));
        let v = OSD::Map(m);
        let bytes = v.to_binary();
        let parsed = OSD::from_binary(&bytes).unwrap();
        assert_eq!(v, parsed);
    }

    #[test]
    fn roundtrip_binary_nested() {
        let mut inner = IndexMap::new();
        inner.insert("x".to_string(), OSD::Integer(10));
        inner.insert("y".to_string(), OSD::Real(2.5));
        let v = OSD::Array(vec![
            OSD::Map(inner),
            OSD::Array(vec![OSD::Integer(1), OSD::Integer(2)]),
            OSD::String("done".to_string()),
        ]);
        let bytes = v.to_binary();
        let parsed = OSD::from_binary(&bytes).unwrap();
        assert_eq!(v, parsed);
    }

    #[test]
    fn parse_binary_without_header() {
        // Integer 42: 'i' + BE bytes
        let data = [b'i', 0, 0, 0, 42];
        let result = parse_binary(&data).unwrap();
        assert_eq!(result, OSD::Integer(42));
    }
}
