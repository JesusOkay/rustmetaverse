use indexmap::IndexMap;
use md5;
use reqwest::{
    header::{ACCEPT, CONTENT_TYPE},
    Client,
};
use rustmetaverse_structured_data::osd::OSD;
use std::io::Cursor;
use std::time::Duration;
use xml::reader::{EventReader, XmlEvent};

pub struct LoginParams {
    pub first_name: String,
    pub last_name: String,
    pub password: String,
    pub start: String,
    pub channel: String,
    pub version: String,
    pub platform: String,
    pub platform_version: String,
    pub mac: String,
    pub id0: String,
    pub viewer_digest: String,
    pub user_agent: String,
    pub author: String,
    pub agree_to_tos: bool,
    pub read_critical: bool,
    pub last_exec_event: i32,
    pub options: Vec<String>,
}

impl Default for LoginParams {
    fn default() -> Self {
        Self {
            first_name: String::new(),
            last_name: String::new(),
            password: String::new(),
            start: "last".to_string(),
            channel: "RustMetaverse".to_string(),
            version: "0.1.0".to_string(),
            platform: "Linux".to_string(),
            platform_version: "0.0.0".to_string(),
            mac: "5f4dcc3b5aa765d61d8327deb882cf99".to_string(), // Random MD5
            id0: "5f4dcc3b5aa765d61d8327deb882cf99".to_string(), // Random MD5
            viewer_digest: String::new(),
            user_agent: "RustMetaverse".to_string(),
            author: "RustMetaverse".to_string(),
            agree_to_tos: true,
            read_critical: true,
            last_exec_event: 0, // Normal
            options: vec![
                "inventory-root".to_string(),
                "inventory-skeleton".to_string(),
                "initial-outfit".to_string(),
                "gestures".to_string(),
                "display_names".to_string(),
                "event_categories".to_string(),
                "event_notifications".to_string(),
                "classified_categories".to_string(),
                "adult_compliant".to_string(),
                "buddy-list".to_string(),
                "newuser-config".to_string(),
                "ui-config".to_string(),
                "advanced-mode".to_string(),
                "max-agent-groups".to_string(),
                "map-server-url".to_string(),
                "voice-config".to_string(),
                "tutorial_settings".to_string(),
                "login-flags".to_string(),
                "global-textures".to_string(),
            ],
        }
    }
}

pub async fn login(
    params: &LoginParams,
    login_uri: &str,
) -> Result<OSD, Box<dyn std::error::Error>> {
    // Construct XML-RPC body
    let mut xml_body = String::from("<?xml version=\"1.0\"?>\n<methodCall>\n<methodName>login_to_simulator</methodName>\n<params>\n<param>\n<value>\n<struct>\n");

    let digest = md5::compute(&params.password);
    let pass_hash = format!("$1${:x}", digest);

    let escape_xml = |s: &str| -> String {
        s.replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;")
            .replace("\"", "&quot;")
            .replace("'", "&apos;")
    };

    let mut add_member = |name: &str, val: &str| {
        xml_body.push_str(&format!(
            "<member><name>{}</name><value><string>{}</string></value></member>\n",
            name,
            escape_xml(val)
        ));
    };

    add_member("first", &params.first_name);
    add_member("last", &params.last_name);
    add_member("passwd", &pass_hash);
    add_member("start", &params.start);
    add_member("channel", &params.channel);
    add_member("version", &params.version);
    add_member("platform", &params.platform);
    add_member("platform_version", &params.platform_version);
    add_member("mac", &params.mac);
    add_member("id0", &params.id0);
    add_member("viewer_digest", &params.viewer_digest);
    add_member("user_agent", &params.user_agent);
    add_member("author", &params.author);

    xml_body.push_str(&format!(
        "<member><name>agree_to_tos</name><value><boolean>{}</boolean></value></member>\n",
        if params.agree_to_tos { "1" } else { "0" }
    ));
    xml_body.push_str(&format!(
        "<member><name>read_critical</name><value><boolean>{}</boolean></value></member>\n",
        if params.read_critical { "1" } else { "0" }
    ));
    xml_body.push_str(&format!(
        "<member><name>last_exec_event</name><value><int>{}</int></value></member>\n",
        params.last_exec_event
    ));

    xml_body.push_str("<member><name>options</name><value><array><data>\n");
    for opt in &params.options {
        xml_body.push_str(&format!(
            "<value><string>{}</string></value>\n",
            escape_xml(opt)
        ));
    }
    xml_body.push_str("</data></array></value></member>\n");

    xml_body.push_str("</struct>\n</value>\n</param>\n</params>\n</methodCall>");

    // Some login frontends return an HTML error page when the request does
    // not identify itself as an XML-RPC client. Use the supplied viewer user
    // agent at the HTTP layer as well as in the XML-RPC payload.
    let client = Client::builder()
        .user_agent(&params.user_agent)
        .timeout(Duration::from_secs(20))
        .build()?;
    let res = client
        .post(login_uri)
        .header("Content-Type", "text/xml")
        .header(ACCEPT, "text/xml, application/xml;q=0.9, */*;q=0.1")
        .body(xml_body)
        .send()
        .await?;

    let status = res.status();
    let content_type = res
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
        .to_owned();
    let text = res.text().await?;

    // Do not feed an HTML proxy/WAF error page to the XML parser: it produces
    // misleading messages such as "BODY != P". Keep the response body out of
    // the error so a login service never leaks details to the terminal.
    if !status.is_success() {
        return Err(std::io::Error::other(format!(
            "login endpoint returned HTTP {status} ({content_type}), not XML-RPC"
        ))
        .into());
    }

    let trimmed = text.trim_start();
    if !(trimmed.starts_with("<?xml") || trimmed.starts_with("<methodResponse")) {
        return Err(std::io::Error::other(format!(
            "login endpoint returned non-XML-RPC content ({content_type})"
        ))
        .into());
    }

    // Parse XML-RPC response
    parse_xml_rpc_response(&text)
}

fn parse_xml_rpc_response(xml: &str) -> Result<OSD, Box<dyn std::error::Error>> {
    let cursor = Cursor::new(xml);
    let parser = EventReader::new(cursor);
    let mut iter = parser.into_iter();

    // Simple state machine to find the struct in methodResponse
    // We expect <methodResponse><params><param><value><struct>...
    // Or <methodResponse><fault><value><struct>...

    loop {
        match iter.next() {
            Some(Ok(XmlEvent::StartElement { name, .. })) => {
                if name.local_name == "struct" {
                    return parse_struct(&mut iter).map_err(|e| e.into());
                }
            }
            Some(Ok(_)) => continue,
            Some(Err(e)) => return Err(Box::new(e)),
            None => return Err("No struct found in response".into()),
        }
    }
}

fn parse_struct<R: std::io::Read>(iter: &mut xml::reader::Events<R>) -> Result<OSD, String> {
    let mut map = IndexMap::new();
    loop {
        match iter.next() {
            Some(Ok(XmlEvent::StartElement { name, .. })) => {
                if name.local_name == "member" {
                    let (key, value) = parse_member(iter)?;
                    map.insert(key, value);
                }
            }
            Some(Ok(XmlEvent::EndElement { name })) => {
                if name.local_name == "struct" {
                    return Ok(OSD::Map(map));
                }
            }
            Some(Ok(_)) => continue,
            Some(Err(e)) => return Err(format!("XML error: {}", e)),
            None => return Err("Unexpected end of XML in struct".to_string()),
        }
    }
}

fn parse_member<R: std::io::Read>(
    iter: &mut xml::reader::Events<R>,
) -> Result<(String, OSD), String> {
    let mut key = String::new();
    let mut value = OSD::Undef;

    loop {
        match iter.next() {
            Some(Ok(XmlEvent::StartElement { name, .. })) => {
                if name.local_name == "name" {
                    key = parse_string_content(iter)?;
                } else if name.local_name == "value" {
                    value = parse_value(iter)?;
                }
            }
            Some(Ok(XmlEvent::EndElement { name })) => {
                if name.local_name == "member" {
                    return Ok((key, value));
                }
            }
            Some(Ok(_)) => continue,
            Some(Err(e)) => return Err(format!("XML error: {}", e)),
            None => return Err("Unexpected end of XML in member".to_string()),
        }
    }
}

fn parse_value<R: std::io::Read>(iter: &mut xml::reader::Events<R>) -> Result<OSD, String> {
    loop {
        match iter.next() {
            Some(Ok(XmlEvent::StartElement { name, .. })) => {
                match name.local_name.as_str() {
                    "string" => return Ok(OSD::String(parse_string_content(iter)?)),
                    "int" | "i4" => {
                        let s = parse_string_content(iter)?;
                        return Ok(OSD::Integer(s.parse().unwrap_or(0)));
                    }
                    "boolean" => {
                        let s = parse_string_content(iter)?;
                        return Ok(OSD::Boolean(s == "1" || s == "true"));
                    }
                    "struct" => return parse_struct(iter),
                    "array" => return parse_array(iter),
                    _ => {
                        // Skip unknown tags or treat as string?
                    }
                }
            }
            Some(Ok(XmlEvent::Characters(s))) => {
                // Untyped value is string
                return Ok(OSD::String(s));
            }
            Some(Ok(XmlEvent::EndElement { name })) => {
                if name.local_name == "value" {
                    // Empty value?
                    return Ok(OSD::Undef);
                }
            }
            Some(Ok(_)) => continue,
            Some(Err(e)) => return Err(format!("XML error: {}", e)),
            None => return Err("Unexpected end of XML in value".to_string()),
        }
    }
}

fn parse_string_content<R: std::io::Read>(
    iter: &mut xml::reader::Events<R>,
) -> Result<String, String> {
    let mut content = String::new();
    loop {
        match iter.next() {
            Some(Ok(XmlEvent::Characters(s))) => content.push_str(&s),
            Some(Ok(XmlEvent::EndElement { .. })) => return Ok(content),
            Some(Ok(_)) => continue,
            Some(Err(e)) => return Err(format!("XML error: {}", e)),
            None => return Err("Unexpected end of XML in string".to_string()),
        }
    }
}

fn parse_array<R: std::io::Read>(iter: &mut xml::reader::Events<R>) -> Result<OSD, String> {
    // <array><data><value>...</value></data></array>
    let mut arr = Vec::new();
    loop {
        match iter.next() {
            Some(Ok(XmlEvent::StartElement { name, .. })) => {
                if name.local_name == "value" {
                    arr.push(parse_value(iter)?);
                }
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
