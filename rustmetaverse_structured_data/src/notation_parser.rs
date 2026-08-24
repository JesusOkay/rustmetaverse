//! LLSD notation serialization and deserialization.
//!
//! Notation LLSD is a human-readable text format. Unlike XML, it uses
//! single-character type prefixes: `i` for integer, `r` for real, `s` for
//! string (with size prefix), `'` or `"` for delimited strings, etc.
//!
//! See <https://wiki.secondlife.com/wiki/LLSD#Notation_Serialization>

use crate::osd::OSD;
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use rustmetaverse_types::UUID;

/// Serialize an [`OSD`] value to notation LLSD string (with header).
pub fn format_notation(value: &OSD) -> String {
    let mut buf = String::with_capacity(256);
    buf.push_str("<?llsd/notation?>\n");
    write_notation(&mut buf, value);
    buf
}

/// Serialize an [`OSD`] value into a string buffer (no header).
pub fn write_notation(buf: &mut String, value: &OSD) {
    use std::fmt::Write;
    match value {
        OSD::Undef => {
            buf.push('!');
        }
        OSD::Boolean(true) => {
            buf.push_str("true");
        }
        OSD::Boolean(false) => {
            buf.push_str("false");
        }
        OSD::Integer(v) => {
            let _ = write!(buf, "i{}", v);
        }
        OSD::Real(v) => {
            if v.is_nan() {
                let _ = write!(buf, "rNaN");
            } else if v.is_infinite() {
                if *v > 0.0 {
                    let _ = write!(buf, "rinf");
                } else {
                    let _ = write!(buf, "r-inf");
                }
            } else {
                let _ = write!(buf, "r{}", v);
            }
        }
        OSD::UUID(v) => {
            let _ = write!(buf, "u\"{}\"", v);
        }
        OSD::String(s) => write_escaped_string(buf, s, '"'),
        OSD::Uri(s) => {
            buf.push('l');
            write_escaped_string(buf, s, '"');
        }
        OSD::Date(d) => {
            let _ = write!(buf, "d\"{}\"", d.to_rfc3339());
        }
        OSD::Binary(b) => {
            // Notation binary: b(size)"raw data"
            // We write hex-encoded since raw binary in text is impractical.
            let hex: String = b.iter().map(|byte| format!("{:02x}", byte)).collect();
            let _ = write!(buf, "b{}\"{}\"", hex.len(), hex);
        }
        OSD::Array(arr) => {
            buf.push('[');
            for (i, item) in arr.iter().enumerate() {
                if i > 0 {
                    buf.push(',');
                }
                write_notation(buf, item);
            }
            buf.push(']');
        }
        OSD::Map(m) => {
            buf.push('{');
            for (i, (k, v)) in m.iter().enumerate() {
                if i > 0 {
                    buf.push(',');
                }
                write_escaped_string(buf, k, '\'');
                buf.push(':');
                write_notation(buf, v);
            }
            buf.push('}');
        }
    }
}

fn write_escaped_string(buf: &mut String, s: &str, delim: char) {
    buf.push(delim);
    for c in s.chars() {
        match c {
            '\\' => buf.push_str("\\\\"),
            c if c == delim => {
                buf.push('\\');
                buf.push(c);
            }
            '\n' => buf.push_str("\\n"),
            '\r' => buf.push_str("\\r"),
            '\t' => buf.push_str("\\t"),
            c => buf.push(c),
        }
    }
    buf.push(delim);
}

/// Parse notation LLSD from a string (with or without header).
pub fn parse_notation(data: &str) -> Result<OSD, String> {
    let data = data.trim_start();
    let data = if data.starts_with("<?llsd/notation?>") {
        let after = &data[18..];
        after.trim_start()
    } else {
        data
    };
    let mut parser = NotationParser {
        chars: data.chars().collect(),
        pos: 0,
    };
    parser.skip_whitespace();
    parser.parse_value()
}

struct NotationParser {
    chars: Vec<char>,
    pos: usize,
}

impl NotationParser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn next_char(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn parse_value(&mut self) -> Result<OSD, String> {
        self.skip_whitespace();
        match self.peek() {
            None => Err("Unexpected end of notation LLSD".to_string()),
            Some(c) => match c {
                '!' => {
                    self.pos += 1;
                    Ok(OSD::Undef)
                }
                '1' => {
                    self.pos += 1;
                    Ok(OSD::Boolean(true))
                }
                '0' => {
                    self.pos += 1;
                    Ok(OSD::Boolean(false))
                }
                't' | 'T' => self.parse_true(),
                'f' | 'F' => self.parse_false(),
                'i' => self.parse_integer(),
                'r' => self.parse_real(),
                'u' => self.parse_uuid(),
                's' => self.parse_sized_string(),
                '\'' | '"' => self.parse_delim_string(),
                'l' => self.parse_uri(),
                'd' => self.parse_date(),
                'b' => self.parse_binary(),
                '[' => self.parse_array(),
                '{' => self.parse_map(),
                _ => Err(format!("Invalid notation token: '{}'", c)),
            },
        }
    }

    fn parse_true(&mut self) -> Result<OSD, String> {
        // Accept t, T, true, TRUE
        let word = self.read_alpha_word();
        match word.as_str() {
            "t" | "T" | "true" | "TRUE" => Ok(OSD::Boolean(true)),
            _ => Err(format!("Invalid boolean: {}", word)),
        }
    }

    fn parse_false(&mut self) -> Result<OSD, String> {
        let word = self.read_alpha_word();
        match word.as_str() {
            "f" | "F" | "false" | "FALSE" => Ok(OSD::Boolean(false)),
            _ => Err(format!("Invalid boolean: {}", word)),
        }
    }

    fn read_alpha_word(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_alphabetic() {
                s.push(c);
                self.pos += 1;
            } else {
                break;
            }
        }
        s
    }

    fn parse_integer(&mut self) -> Result<OSD, String> {
        self.pos += 1; // skip 'i'
        let s = self.read_number_str();
        let v: i32 = s.parse().map_err(|_| format!("Invalid integer: {}", s))?;
        Ok(OSD::Integer(v))
    }

    fn parse_real(&mut self) -> Result<OSD, String> {
        self.pos += 1; // skip 'r'
                       // Check for special values
        let word = self.peek_alpha_word();
        if !word.is_empty() {
            self.pos += word.len();
            match word.as_str() {
                "NaN" | "nan" => return Ok(OSD::Real(f64::NAN)),
                "inf" | "Inf" | "INF" => return Ok(OSD::Real(f64::INFINITY)),
                "-inf" | "-Inf" => return Ok(OSD::Real(f64::NEG_INFINITY)),
                _ => {}
            }
        }
        let s = self.read_number_str();
        let v: f64 = s.parse().map_err(|_| format!("Invalid real: {}", s))?;
        Ok(OSD::Real(v))
    }

    fn peek_alpha_word(&self) -> String {
        let mut s = String::new();
        let mut i = self.pos;
        while let Some(&c) = self.chars.get(i) {
            if c.is_ascii_alphabetic() || c == '-' {
                s.push(c);
                i += 1;
            } else {
                break;
            }
        }
        s
    }

    fn read_number_str(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '-' || c == '+' || c == '.' || c == 'e' || c == 'E' {
                s.push(c);
                self.pos += 1;
            } else {
                break;
            }
        }
        s
    }

    fn parse_uuid(&mut self) -> Result<OSD, String> {
        self.pos += 1; // skip 'u'
        self.skip_whitespace();
        // Expect quoted string
        let s = if self.peek() == Some('"') || self.peek() == Some('\'') {
            self.parse_delim_string()?.as_string().unwrap_or_default()
        } else {
            // Raw UUID without quotes (less common)
            let mut s = String::new();
            while let Some(c) = self.peek() {
                if c.is_ascii_hexdigit() || c == '-' {
                    s.push(c);
                    self.pos += 1;
                } else {
                    break;
                }
            }
            s
        };
        let uuid = UUID::parse(&s).map_err(|_| format!("Invalid UUID: {}", s))?;
        Ok(OSD::UUID(uuid))
    }

    fn parse_sized_string(&mut self) -> Result<OSD, String> {
        self.pos += 1; // skip 's'
                       // s(size)"data"
        let size_str = self.read_number_str();
        let _size: usize = size_str
            .parse()
            .map_err(|_| format!("Invalid string size: {}", size_str))?;
        self.skip_whitespace();
        let s = self.parse_delim_string_body()?;
        Ok(OSD::String(s))
    }

    fn parse_delim_string(&mut self) -> Result<OSD, String> {
        let delim = self.next_char().ok_or("Unexpected end of string")?;
        let s = self.parse_delimited(delim)?;
        Ok(OSD::String(s))
    }

    fn parse_delim_string_body(&mut self) -> Result<String, String> {
        let delim = self.next_char().ok_or("Unexpected end of string")?;
        self.parse_delimited(delim)
    }

    fn parse_delimited(&mut self, delim: char) -> Result<String, String> {
        let mut s = String::new();
        loop {
            match self.next_char() {
                None => return Err("Unterminated delimited string".to_string()),
                Some('\\') => match self.next_char() {
                    Some('n') => s.push('\n'),
                    Some('r') => s.push('\r'),
                    Some('t') => s.push('\t'),
                    Some('\\') => s.push('\\'),
                    Some(c) if c == delim => s.push(c),
                    Some(c) => {
                        s.push('\\');
                        s.push(c);
                    }
                    None => return Err("Unterminated escape".to_string()),
                },
                Some(c) if c == delim => break,
                Some(c) => s.push(c),
            }
        }
        Ok(s)
    }

    fn parse_uri(&mut self) -> Result<OSD, String> {
        self.pos += 1; // skip 'l'
        self.skip_whitespace();
        let s = self.parse_delim_string_body()?;
        Ok(OSD::Uri(s))
    }

    fn parse_date(&mut self) -> Result<OSD, String> {
        self.pos += 1; // skip 'd'
        self.skip_whitespace();
        let s = self.parse_delim_string_body()?;
        let dt: DateTime<Utc> = s.parse().map_err(|_| format!("Invalid date: {}", s))?;
        Ok(OSD::Date(dt))
    }

    fn parse_binary(&mut self) -> Result<OSD, String> {
        self.pos += 1; // skip 'b'
                       // b(size)"hexdata" or b"hexdata" (size optional, but we read it if present)
        let _size_str = self.read_number_str();
        self.skip_whitespace();
        let hex = self.parse_delim_string_body()?;
        let bytes = hex_to_bytes(&hex)?;
        Ok(OSD::Binary(bytes))
    }

    fn parse_array(&mut self) -> Result<OSD, String> {
        self.pos += 1; // skip '['
        let mut arr = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(OSD::Array(arr));
        }
        loop {
            self.skip_whitespace();
            arr.push(self.parse_value()?);
            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.pos += 1;
                }
                Some(']') => {
                    self.pos += 1;
                    break;
                }
                None => return Err("Unterminated array".to_string()),
                Some(c) => return Err(format!("Expected ',' or ']' in array, got '{}'", c)),
            }
        }
        Ok(OSD::Array(arr))
    }

    fn parse_map(&mut self) -> Result<OSD, String> {
        self.pos += 1; // skip '{'
        let mut map = IndexMap::new();
        self.skip_whitespace();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(OSD::Map(map));
        }
        loop {
            self.skip_whitespace();
            // Key is a string (delimited or sized)
            let key = match self.peek() {
                Some('\'') | Some('"') => {
                    let delim = self.next_char().unwrap();
                    self.parse_delimited(delim)?
                }
                Some('s') => {
                    self.pos += 1;
                    self.read_number_str();
                    self.skip_whitespace();
                    self.parse_delim_string_body()?
                }
                _ => return Err("Expected string key in map".to_string()),
            };
            self.skip_whitespace();
            if self.peek() != Some(':') {
                return Err("Expected ':' after map key".to_string());
            }
            self.pos += 1; // skip ':'
            self.skip_whitespace();
            let value = self.parse_value()?;
            map.insert(key, value);
            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.pos += 1;
                }
                Some('}') => {
                    self.pos += 1;
                    break;
                }
                None => return Err("Unterminated map".to_string()),
                Some(c) => return Err(format!("Expected ',' or '}}' in map, got '{}'", c)),
            }
        }
        Ok(OSD::Map(map))
    }
}

fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err("Hex string has odd length".to_string());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| format!("Invalid hex byte: {}", &hex[i..i + 2]))
        })
        .collect()
}

impl OSD {
    /// Serialize to notation LLSD string (with header).
    pub fn to_notation(&self) -> String {
        format_notation(self)
    }

    /// Parse notation LLSD from a string.
    pub fn from_notation(data: &str) -> Result<Self, String> {
        parse_notation(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_notation_scalars() {
        let values = vec![
            OSD::Undef,
            OSD::Boolean(true),
            OSD::Boolean(false),
            OSD::Integer(42),
            OSD::Integer(-1),
            OSD::Real(42.195),
            OSD::String("hello".to_string()),
            OSD::String("say \"hi\"".to_string()),
            OSD::Binary(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        ];
        for v in values {
            let notation = format_notation(&v);
            let parsed = parse_notation(&notation).unwrap();
            assert_eq!(v, parsed, "roundtrip failed for {:?}", v);
        }
    }

    #[test]
    fn roundtrip_notation_array() {
        let v = OSD::Array(vec![
            OSD::Integer(1),
            OSD::String("two".to_string()),
            OSD::Boolean(true),
        ]);
        let notation = format_notation(&v);
        let parsed = parse_notation(&notation).unwrap();
        assert_eq!(v, parsed);
    }

    #[test]
    fn roundtrip_notation_map() {
        let mut m = IndexMap::new();
        m.insert("name".to_string(), OSD::String("test".to_string()));
        m.insert("count".to_string(), OSD::Integer(7));
        let v = OSD::Map(m);
        let notation = format_notation(&v);
        let parsed = parse_notation(&notation).unwrap();
        assert_eq!(v, parsed);
    }

    #[test]
    fn parse_notation_bare_bool() {
        assert_eq!(parse_notation("!").unwrap(), OSD::Undef);
        assert_eq!(parse_notation("true").unwrap(), OSD::Boolean(true));
        assert_eq!(parse_notation("false").unwrap(), OSD::Boolean(false));
    }

    #[test]
    fn parse_notation_with_header() {
        let data = "<?llsd/notation?>\ni42";
        assert_eq!(parse_notation(data).unwrap(), OSD::Integer(42));
    }
}
