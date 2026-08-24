use crate::osd::OSD;
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use rustmetaverse_types::UUID;
use std::io::Cursor;
use xml::reader::{EventReader, Events, XmlEvent};

pub fn parse_xml(xml_content: &str) -> Result<OSD, String> {
    let cursor = Cursor::new(xml_content);
    let parser = EventReader::new(cursor);
    let mut iter = parser.into_iter();

    // Skip until we find <llsd>
    loop {
        match iter.next() {
            Some(Ok(XmlEvent::StartElement { name, .. })) => {
                if name.local_name == "llsd" {
                    break;
                }
            }
            Some(Ok(_)) => continue,
            Some(Err(e)) => return Err(format!("XML error: {}", e)),
            None => return Err("No llsd tag found".to_string()),
        }
    }

    // Now parse the content
    parse_element(&mut iter)
}

fn parse_element<R: std::io::Read>(iter: &mut Events<R>) -> Result<OSD, String> {
    loop {
        match iter.next() {
            Some(Ok(XmlEvent::StartElement { name, .. })) => {
                return parse_value_from_name(name.local_name.as_str(), iter);
            }
            Some(Ok(XmlEvent::EndElement { .. })) => {
                // Could be empty llsd?
                return Ok(OSD::Undef);
            }
            Some(Ok(_)) => continue,
            Some(Err(e)) => return Err(format!("XML error: {}", e)),
            None => return Err("Unexpected end of XML".to_string()),
        }
    }
}

fn parse_value_from_name<R: std::io::Read>(
    name: &str,
    iter: &mut Events<R>,
) -> Result<OSD, String> {
    match name {
        "undef" => {
            parse_content(iter)?;
            Ok(OSD::Undef)
        }
        "boolean" => parse_boolean(iter),
        "integer" => parse_integer(iter),
        "real" => parse_real(iter),
        "uuid" => parse_uuid(iter),
        "string" => parse_string(iter),
        "binary" => parse_binary(iter),
        "date" => parse_date(iter),
        "uri" => parse_uri(iter),
        "map" => parse_map(iter),
        "array" => parse_array(iter),
        _ => Err(format!("Unknown element: {}", name)),
    }
}

fn parse_content<R: std::io::Read>(iter: &mut Events<R>) -> Result<String, String> {
    let mut content = String::new();
    loop {
        match iter.next() {
            Some(Ok(XmlEvent::Characters(s))) => content.push_str(&s),
            Some(Ok(XmlEvent::EndElement { .. })) => return Ok(content),
            Some(Ok(XmlEvent::StartElement { .. })) => {
                return Err("Unexpected nested element in scalar".to_string())
            }
            Some(Ok(_)) => continue,
            Some(Err(e)) => return Err(format!("XML error: {}", e)),
            None => return Err("Unexpected end of XML".to_string()),
        }
    }
}

fn parse_boolean<R: std::io::Read>(iter: &mut Events<R>) -> Result<OSD, String> {
    let s = parse_content(iter)?;
    match s.trim() {
        "1" | "true" => Ok(OSD::Boolean(true)),
        _ => Ok(OSD::Boolean(false)),
    }
}

fn parse_integer<R: std::io::Read>(iter: &mut Events<R>) -> Result<OSD, String> {
    let s = parse_content(iter)?;
    if s.trim().is_empty() {
        return Ok(OSD::Integer(0));
    }
    let i = s
        .trim()
        .parse::<i32>()
        .map_err(|_| "Invalid integer".to_string())?;
    Ok(OSD::Integer(i))
}

fn parse_real<R: std::io::Read>(iter: &mut Events<R>) -> Result<OSD, String> {
    let s = parse_content(iter)?;
    if s.trim().is_empty() {
        return Ok(OSD::Real(0.0));
    }
    let f = s
        .trim()
        .parse::<f64>()
        .map_err(|_| "Invalid real".to_string())?;
    Ok(OSD::Real(f))
}

fn parse_uuid<R: std::io::Read>(iter: &mut Events<R>) -> Result<OSD, String> {
    let s = parse_content(iter)?;
    if s.trim().is_empty() {
        return Ok(OSD::UUID(UUID::ZERO));
    }
    let uuid = UUID::parse(s.trim()).map_err(|_| "Invalid UUID".to_string())?;
    Ok(OSD::UUID(uuid))
}

fn parse_string<R: std::io::Read>(iter: &mut Events<R>) -> Result<OSD, String> {
    let s = parse_content(iter)?;
    Ok(OSD::String(s))
}

fn parse_binary<R: std::io::Read>(iter: &mut Events<R>) -> Result<OSD, String> {
    let s = parse_content(iter)?;
    if s.trim().is_empty() {
        return Ok(OSD::Binary(Vec::new()));
    }
    let bytes = general_purpose::STANDARD
        .decode(s.trim())
        .map_err(|_| "Invalid base64".to_string())?;
    Ok(OSD::Binary(bytes))
}

fn parse_date<R: std::io::Read>(iter: &mut Events<R>) -> Result<OSD, String> {
    let s = parse_content(iter)?;
    if s.trim().is_empty() {
        // Return epoch?
        return Ok(OSD::Date(DateTime::<Utc>::MIN_UTC));
    }
    let date = s
        .trim()
        .parse::<DateTime<Utc>>()
        .map_err(|_| "Invalid date".to_string())?;
    Ok(OSD::Date(date))
}

fn parse_uri<R: std::io::Read>(iter: &mut Events<R>) -> Result<OSD, String> {
    let s = parse_content(iter)?;
    Ok(OSD::Uri(s))
}

fn parse_map<R: std::io::Read>(iter: &mut Events<R>) -> Result<OSD, String> {
    let mut map = IndexMap::new();
    let mut current_key = None;

    loop {
        match iter.next() {
            Some(Ok(XmlEvent::StartElement { name, .. })) => {
                if name.local_name == "key" {
                    current_key = Some(parse_content(iter)?);
                } else {
                    if let Some(key) = current_key.take() {
                        let val = parse_value_from_name(name.local_name.as_str(), iter)?;
                        map.insert(key, val);
                    } else {
                        return Err("Value without key in map".to_string());
                    }
                }
            }
            Some(Ok(XmlEvent::EndElement { name })) => {
                if name.local_name == "map" {
                    return Ok(OSD::Map(map));
                }
            }
            Some(Ok(_)) => continue,
            Some(Err(e)) => return Err(format!("XML error: {}", e)),
            None => return Err("Unexpected end of XML in map".to_string()),
        }
    }
}

fn parse_array<R: std::io::Read>(iter: &mut Events<R>) -> Result<OSD, String> {
    let mut arr = Vec::new();
    loop {
        match iter.next() {
            Some(Ok(XmlEvent::StartElement { name, .. })) => {
                let val = parse_value_from_name(name.local_name.as_str(), iter)?;
                arr.push(val);
            }
            Some(Ok(XmlEvent::EndElement { name })) => {
                if name.local_name == "array" {
                    return Ok(OSD::Array(arr));
                }
            }
            Some(Ok(_)) => continue,
            Some(Err(e)) => return Err(format!("XML error: {}", e)),
            None => return Err("Unexpected end of XML in array".to_string()),
        }
    }
}
