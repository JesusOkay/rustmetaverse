// use std::collections::HashMap;
// use std::fmt;
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use rustmetaverse_types::UUID;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum OSD {
    Boolean(bool),
    Integer(i32),
    Real(f64),
    String(String),
    UUID(UUID),
    Date(DateTime<Utc>),
    Uri(String), // Keeping as String for simplicity for now
    Binary(Vec<u8>),
    Map(IndexMap<String, OSD>),
    Array(Vec<OSD>),
    #[default]
    Undef,
}

impl OSD {
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            OSD::Boolean(v) => Some(*v),
            OSD::Integer(v) => Some(*v != 0),
            OSD::String(v) => {
                if v == "1" || v.eq_ignore_ascii_case("true") {
                    Some(true)
                } else if v == "0" || v.eq_ignore_ascii_case("false") {
                    Some(false)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<i32> {
        match self {
            OSD::Integer(v) => Some(*v),
            OSD::Real(v) => Some(*v as i32),
            OSD::Boolean(v) => Some(if *v { 1 } else { 0 }),
            OSD::String(v) => v.parse().ok(),
            _ => None,
        }
    }

    pub fn as_real(&self) -> Option<f64> {
        match self {
            OSD::Real(v) => Some(*v),
            OSD::Integer(v) => Some(*v as f64),
            OSD::Boolean(v) => Some(if *v { 1.0 } else { 0.0 }),
            OSD::String(v) => v.parse().ok(),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<String> {
        match self {
            OSD::String(v) => Some(v.clone()),
            OSD::Boolean(v) => Some(if *v { "1".to_string() } else { "0".to_string() }),
            OSD::Integer(v) => Some(v.to_string()),
            OSD::Real(v) => Some(v.to_string()),
            OSD::UUID(v) => Some(v.to_string()),
            OSD::Uri(v) => Some(v.clone()),
            OSD::Date(v) => Some(v.to_rfc3339()),
            _ => None,
        }
    }

    pub fn as_uuid(&self) -> Option<UUID> {
        match self {
            OSD::UUID(v) => Some(*v),
            OSD::String(v) => UUID::parse(v).ok(),
            _ => None,
        }
    }

    pub fn as_date(&self) -> Option<&DateTime<Utc>> {
        match self {
            OSD::Date(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_binary(&self) -> Option<&Vec<u8>> {
        match self {
            OSD::Binary(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_uri(&self) -> Option<&String> {
        match self {
            OSD::Uri(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&IndexMap<String, OSD>> {
        match self {
            OSD::Map(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_map_mut(&mut self) -> Option<&mut IndexMap<String, OSD>> {
        match self {
            OSD::Map(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<OSD>> {
        match self {
            OSD::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut Vec<OSD>> {
        match self {
            OSD::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn is_undef(&self) -> bool {
        matches!(self, OSD::Undef)
    }

    pub fn to_xml(&self) -> String {
        let mut output = String::with_capacity(256);
        output.push_str("<llsd>");
        self.write_xml(&mut output);
        output.push_str("</llsd>");
        output
    }

    fn write_xml(&self, output: &mut String) {
        use std::fmt::Write;
        match self {
            OSD::Boolean(v) => {
                output.push_str("<boolean>");
                output.push_str(if *v { "1" } else { "0" });
                output.push_str("</boolean>");
            }
            OSD::Integer(v) => {
                let _ = write!(output, "<integer>{}</integer>", v);
            }
            OSD::Real(v) => {
                let _ = write!(output, "<real>{}</real>", v);
            }
            OSD::String(v) => {
                output.push_str("<string>");
                Self::escape_xml_into(v, output);
                output.push_str("</string>");
            }
            OSD::UUID(v) => {
                let _ = write!(output, "<uuid>{}</uuid>", v);
            }
            OSD::Date(v) => {
                let _ = write!(output, "<date>{}</date>", v.to_rfc3339());
            }
            OSD::Uri(v) => {
                output.push_str("<uri>");
                Self::escape_xml_into(v, output);
                output.push_str("</uri>");
            }
            OSD::Binary(v) => {
                use base64::{engine::general_purpose, Engine as _};
                output.push_str("<binary>");
                general_purpose::STANDARD.encode_string(v, output);
                output.push_str("</binary>");
            }
            OSD::Map(m) => {
                output.push_str("<map>");
                for (k, v) in m {
                    output.push_str("<key>");
                    Self::escape_xml_into(k, output);
                    output.push_str("</key>");
                    v.write_xml(output);
                }
                output.push_str("</map>");
            }
            OSD::Array(a) => {
                output.push_str("<array>");
                for v in a {
                    v.write_xml(output);
                }
                output.push_str("</array>");
            }
            OSD::Undef => output.push_str("<undef />"),
        }
    }

    fn escape_xml_into(s: &str, output: &mut String) {
        // Preallocate roughly; most strings have no special chars.
        if !s.contains(['&', '<', '>', '"', '\'']) {
            output.push_str(s);
            return;
        }
        for c in s.chars() {
            match c {
                '&' => output.push_str("&amp;"),
                '<' => output.push_str("&lt;"),
                '>' => output.push_str("&gt;"),
                '"' => output.push_str("&quot;"),
                '\'' => output.push_str("&apos;"),
                _ => output.push(c),
            }
        }
    }
}

// Helper macros or functions for creating OSDs could go here
