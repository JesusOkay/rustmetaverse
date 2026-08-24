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

    pub fn to_xml(&self) -> String {
        let mut output = String::new();
        output.push_str("<llsd>");
        self.write_xml(&mut output);
        output.push_str("</llsd>");
        output
    }

    fn write_xml(&self, output: &mut String) {
        match self {
            OSD::Boolean(v) => output.push_str(&format!(
                "<boolean>{}</boolean>",
                if *v { "1" } else { "0" }
            )),
            OSD::Integer(v) => output.push_str(&format!("<integer>{}</integer>", v)),
            OSD::Real(v) => output.push_str(&format!("<real>{}</real>", v)),
            OSD::String(v) => output.push_str(&format!("<string>{}</string>", Self::escape_xml(v))),
            OSD::UUID(v) => output.push_str(&format!("<uuid>{}</uuid>", v)),
            OSD::Date(v) => output.push_str(&format!("<date>{}</date>", v.to_rfc3339())),
            OSD::Uri(v) => output.push_str(&format!("<uri>{}</uri>", Self::escape_xml(v))),
            OSD::Binary(v) => {
                use base64::{engine::general_purpose, Engine as _};
                let encoded = general_purpose::STANDARD.encode(v);
                output.push_str(&format!("<binary>{}</binary>", encoded));
            }
            OSD::Map(m) => {
                output.push_str("<map>");
                for (k, v) in m {
                    output.push_str(&format!("<key>{}</key>", Self::escape_xml(k)));
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

    fn escape_xml(s: &str) -> String {
        s.replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;")
            .replace("\"", "&quot;")
            .replace("'", "&apos;")
    }
}

// Helper macros or functions for creating OSDs could go here
