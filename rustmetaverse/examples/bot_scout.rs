//! Bot Scout - Single bot test
//! Connects bot to specific region, scans for resident
//!
//! Usage:
//!   cargo run --release --example bot_scout -- \
//!       BotFirst BotLast BotPassword RegionName TargetFirst TargetLast
//!
//! Example:
//!   cargo run --release --example bot_scout -- \
//!       bot1 Resident password123 "Help Island" John Resident

use indexmap::IndexMap;
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use rustmetaverse::{GridClient, LoginParams};
use rustmetaverse_protocol::header::{Header, PacketFrequency};
use rustmetaverse_protocol::packets::{
    PacketType, UUIDNameRequestPacket, UUIDNameRequestUUIDNameBlockBlock, WrappedPacket,
};
use rustmetaverse_structured_data::{xml_parser::parse_xml, OSD};
use rustmetaverse_types::UUID;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration, Instant};

const LOGIN_URI: &str = "https://login.agni.lindenlab.com/cgi-bin/login.cgi";
const COARSE_UPDATE_TIMEOUT: Duration = Duration::from_secs(10);
const NAME_LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(30);
const REGION_READY_TIMEOUT: Duration = Duration::from_secs(15);
const CAPABILITY_TIMEOUT: Duration = Duration::from_secs(15);
const LLSD_XML: &str = "application/llsd+xml";

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const CYAN: &str = "\x1b[36m";
const YELLOW: &str = "\x1b[33m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

fn random_mac_id0() -> String {
    use rand::Rng;
    let bytes: [u8; 16] = rand::thread_rng().gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[derive(Clone, Debug)]
struct AvatarCacheEntry {
    first_name: String,
    last_name: String,
    agent_id: UUID,
}

#[derive(Default)]
struct CoarseLocationSnapshot {
    received: bool,
    avatars: HashSet<UUID>,
}

type AvatarNames = Arc<Mutex<HashMap<UUID, AvatarCacheEntry>>>;
type CoarseLocations = Arc<Mutex<CoarseLocationSnapshot>>;

/// The three values exposed by Firestorm's "I want to access content rated"
/// preference. The server protocol represents them as PG, M and A.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContentAccess {
    General,
    Moderate,
    Adult,
}

impl ContentAccess {
    fn from_server_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "PG" => Some(Self::General),
            "M" => Some(Self::Moderate),
            "A" => Some(Self::Adult),
            _ => None,
        }
    }

    fn server_value(self) -> &'static str {
        match self {
            Self::General => "PG",
            Self::Moderate => "M",
            Self::Adult => "A",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Moderate => "General and Moderate",
            Self::Adult => "General, Moderate and Adult",
        }
    }
}

fn osd_map(entries: impl IntoIterator<Item = (String, OSD)>) -> OSD {
    let mut map = IndexMap::new();
    map.extend(entries);
    OSD::Map(map)
}

fn update_agent_information_url(response_body: &str) -> Result<String, String> {
    parse_xml(response_body)
        .map_err(|error| format!("invalid LLSD response from seed capability: {error}"))?
        .as_map()
        .and_then(|capabilities| capabilities.get("UpdateAgentInformation"))
        .and_then(OSD::as_string)
        .filter(|url| !url.trim().is_empty())
        .ok_or_else(|| "the region did not offer UpdateAgentInformation".to_owned())
}

fn content_access_from_response(response_body: &str) -> Result<ContentAccess, String> {
    parse_xml(response_body)
        .map_err(|error| format!("invalid LLSD response from UpdateAgentInformation: {error}"))?
        .as_map()
        .and_then(|result| result.get("access_prefs"))
        .and_then(OSD::as_map)
        .and_then(|access_prefs| access_prefs.get("max"))
        .and_then(OSD::as_string)
        .and_then(|actual| ContentAccess::from_server_value(&actual))
        .ok_or_else(|| "UpdateAgentInformation did not return access_prefs.max".to_owned())
}

async fn update_content_access(
    seed_capability: &str,
    requested: ContentAccess,
) -> Result<ContentAccess, String> {
    if seed_capability.trim().is_empty() {
        return Err("the login response does not contain a seed capability".to_owned());
    }

    let http = reqwest::Client::builder()
        .user_agent("RustMetaverse Bot Scout")
        .timeout(CAPABILITY_TIMEOUT)
        .build()
        .map_err(|error| format!("could not create the HTTP client: {error}"))?;

    // Firestorm asks the region seed for this URL before posting the setting.
    let seed_request = OSD::Array(vec![OSD::String("UpdateAgentInformation".to_owned())]).to_xml();
    let response = http
        .post(seed_capability)
        .header(CONTENT_TYPE, LLSD_XML)
        .header(ACCEPT, LLSD_XML)
        .body(seed_request)
        .send()
        .await
        .map_err(|error| format!("capability request failed: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("could not read the capability response: {error}"))?;
    if !status.is_success() {
        return Err(format!("seed capability returned HTTP {status}"));
    }

    let update_url = update_agent_information_url(&body)?;

    let access_prefs = osd_map([(
        "max".to_owned(),
        OSD::String(requested.server_value().to_owned()),
    )]);
    let update_request = osd_map([("access_prefs".to_owned(), access_prefs)]).to_xml();
    let response = http
        .post(update_url)
        .header(CONTENT_TYPE, LLSD_XML)
        .header(ACCEPT, LLSD_XML)
        .body(update_request)
        .send()
        .await
        .map_err(|error| format!("UpdateAgentInformation failed: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("could not read UpdateAgentInformation: {error}"))?;
    if !status.is_success() {
        return Err(format!("UpdateAgentInformation returned HTTP {status}"));
    }

    content_access_from_response(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_firestorm_capability_and_maturity_responses() {
        let seed_response = r#"<?xml version="1.0"?><llsd><map><key>UpdateAgentInformation</key><uri>https://caps.example.invalid/update</uri></map></llsd>"#;
        assert_eq!(
            update_agent_information_url(seed_response).unwrap(),
            "https://caps.example.invalid/update"
        );

        let update_response = r#"<llsd><map><key>access_prefs</key><map><key>max</key><string>A</string></map></map></llsd>"#;
        assert_eq!(
            content_access_from_response(update_response).unwrap(),
            ContentAccess::Adult
        );
    }

    #[test]
    fn writes_the_same_max_values_as_firestorm() {
        assert_eq!(ContentAccess::General.server_value(), "PG");
        assert_eq!(ContentAccess::Moderate.server_value(), "M");
        assert_eq!(ContentAccess::Adult.server_value(), "A");
    }
}

fn decode_name(value: &[u8]) -> String {
    let end = value
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(value.len());
    String::from_utf8_lossy(&value[..end]).trim().to_owned()
}

async fn scan_region(
    client: &GridClient,
    agent_id: UUID,
    region_name: &str,
    target_first: &str,
    target_last: &str,
    coarse_locations: &CoarseLocations,
    avatar_names: &AvatarNames,
) -> Option<UUID> {
    eprintln!(
        "{}[SCAN]{} Region: {} | Target: {} {}",
        CYAN, RESET, region_name, target_first, target_last
    );

    // Firestorm's "Nearby" list is populated from the simulator's coarse
    // location feed. DirFindQuery is a global directory search and is not a
    // reliable source of avatars currently present in this region.
    eprintln!(
        "{}[WAIT]{} Waiting for the nearby-avatar snapshot...",
        YELLOW, RESET
    );
    let deadline = Instant::now() + COARSE_UPDATE_TIMEOUT;
    let avatar_ids = loop {
        let snapshot = coarse_locations.lock().await;
        if snapshot.received {
            break snapshot
                .avatars
                .iter()
                .copied()
                .filter(|id| *id != agent_id)
                .collect::<Vec<_>>();
        }
        drop(snapshot);

        if Instant::now() >= deadline {
            eprintln!(
                "{}[TIMEOUT]{} The simulator did not send a nearby-avatar snapshot in {}s",
                RED,
                RESET,
                COARSE_UPDATE_TIMEOUT.as_secs()
            );
            return None;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };

    if avatar_ids.is_empty() {
        eprintln!("{}[RESULTS]{} No other avatars are nearby", CYAN, RESET);
        return None;
    }

    // The coarse update contains UUIDs and positions, not names. Resolve the
    // UUIDs through the same legacy name-cache request used by the viewer.
    for ids in avatar_ids.chunks(100) {
        let mut request = UUIDNameRequestPacket {
            header: Header {
                reliable: true,
                sequence: client.network.get_next_sequence(),
                id: 235,
                frequency: PacketFrequency::Low,
                ..Default::default()
            },
            u_u_i_d_name_block: ids
                .iter()
                .copied()
                .map(|i_d| UUIDNameRequestUUIDNameBlockBlock { i_d })
                .collect(),
        };

        if let Err(error) = client.network.send_packet(&mut request).await {
            eprintln!(
                "{}[ERROR]{} Failed to request nearby-avatar names: {}",
                RED, RESET, error
            );
            return None;
        }
    }

    eprintln!("{}[WAIT]{} Resolving nearby-avatar names...", YELLOW, RESET);
    let deadline = Instant::now() + NAME_LOOKUP_TIMEOUT;
    loop {
        let resolved = {
            let names = avatar_names.lock().await;
            avatar_ids
                .iter()
                .filter(|id| names.contains_key(id))
                .count()
        };

        if resolved == avatar_ids.len() || Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let names = avatar_names.lock().await;

    eprintln!(
        "{}[RESULTS]{} {} avatars detected",
        CYAN,
        RESET,
        avatar_ids.len()
    );
    eprintln!();

    for id in &avatar_ids {
        match names.get(id) {
            Some(avatar) => eprintln!(
                "  {} {}  ({})",
                avatar.first_name, avatar.last_name, avatar.agent_id
            ),
            None => eprintln!("  <name pending>  ({})", id),
        }
    }
    eprintln!();

    avatar_ids.iter().find_map(|id| {
        let avatar = names.get(id)?;
        (avatar.first_name.eq_ignore_ascii_case(target_first)
            && avatar.last_name.eq_ignore_ascii_case(target_last))
        .then_some(avatar.agent_id)
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    eprintln!();
    eprintln!(
        "{}{}================================================================================",
        BOLD, CYAN
    );
    eprintln!("                        BOT SCOUT v1.0                                  ");
    eprintln!("              Single Bot Region Scan + Target Detection                 ");
    eprintln!(
        "================================================================================{}",
        RESET
    );
    eprintln!();

    let args: Vec<String> = std::env::args().collect();

    if args.len() != 7 {
        eprintln!(
            "{}[ERROR]{} Usage: {} <bot_first> <bot_last> <bot_pass> <region> <target_first> <target_last>",
            RED, RESET, args[0]
        );
        eprintln!();
        eprintln!("Example:");
        eprintln!(
            "  {} bot1 Resident password123 \"Help Island\" John Resident",
            args[0]
        );
        return Ok(());
    }

    let bot_first = args[1].clone();
    let bot_last = args[2].clone();
    let bot_pass = args[3].clone();
    let region = args[4].clone();
    let target_first = args[5].clone();
    let target_last = args[6].clone();
    // Always request Firestorm's top option as soon as the first region is
    // ready. There is intentionally no command-line switch for this setting.
    let requested_access = ContentAccess::Adult;

    eprintln!("{}CONFIG{}", BOLD, RESET);
    eprintln!("{}-----------{}", DIM, RESET);
    eprintln!("  Bot ........... {} {}", bot_first, bot_last);
    eprintln!("  Region ....... {}", region);
    eprintln!("  Target ....... {} {}", target_first, target_last);
    eprintln!("  Content ...... {}", requested_access.label());
    eprintln!();

    eprintln!("{}[CONNECT]{} Logging in...", CYAN, RESET);

    let client = GridClient::new().await?;

    // Register the nearby-avatar handlers before login. The initial coarse
    // update can arrive while the region handshake is still in progress.
    let coarse_locations: CoarseLocations = Arc::new(Mutex::new(CoarseLocationSnapshot::default()));
    let avatar_names: AvatarNames = Arc::new(Mutex::new(HashMap::new()));

    {
        let mut dispatcher = client.dispatcher.write().await;
        let coarse_locations = coarse_locations.clone();

        dispatcher.add_handler(PacketType::CoarseLocationUpdate, move |packet, _, _| {
            let coarse_locations = coarse_locations.clone();

            async move {
                let WrappedPacket::CoarseLocationUpdate(update) = packet else {
                    return;
                };

                let you_index = usize::try_from(update.index.you).ok();
                let avatars = update
                    .agent_data
                    .iter()
                    .enumerate()
                    .filter_map(|(index, entry)| {
                        (Some(index) != you_index && !entry.agent_i_d.is_zero())
                            .then_some(entry.agent_i_d)
                    })
                    .collect();

                let mut snapshot = coarse_locations.lock().await;
                snapshot.received = true;
                snapshot.avatars = avatars;
            }
        });

        let avatar_names = avatar_names.clone();
        dispatcher.add_handler(PacketType::UUIDNameReply, move |packet, _, _| {
            let avatar_names = avatar_names.clone();

            async move {
                let WrappedPacket::UUIDNameReply(reply) = packet else {
                    return;
                };

                let mut names = avatar_names.lock().await;
                for entry in reply.u_u_i_d_name_block {
                    if entry.i_d.is_zero() {
                        continue;
                    }
                    names.insert(
                        entry.i_d,
                        AvatarCacheEntry {
                            first_name: decode_name(&entry.first_name),
                            last_name: decode_name(&entry.last_name),
                            agent_id: entry.i_d,
                        },
                    );
                }
            }
        });
    }

    let params = LoginParams {
        first_name: bot_first.clone(),
        last_name: bot_last.clone(),
        password: bot_pass.clone(),
        start: format!("uri:{}&128&128&30", region),
        mac: random_mac_id0(),
        id0: random_mac_id0(),
        ..Default::default()
    };

    match timeout(LOGIN_TIMEOUT, client.login_silent(&params, LOGIN_URI)).await {
        Err(_) => {
            eprintln!("{}[FAIL]{} Login timeout", RED, RESET);
            return Ok(());
        }
        Ok(Err(e)) => {
            let error = e.to_string();
            eprintln!("{}[FAIL]{} {}", RED, RESET, error);
            if error.contains("HTTP 403") {
                eprintln!(
                    "{}[BLOCKED]{} The login gateway rejected this network before it processed XML-RPC or credentials.",
                    YELLOW, RESET
                );
                eprintln!(
                    "          This is not a password, region, or content-access error. Check the network path and the grid's client-access policy."
                );
            }
            return Ok(());
        }
        Ok(Ok(())) => {
            eprintln!("{}[OK]{} Connected", GREEN, RESET);
        }
    }

    let (agent_id, seed_capability) = {
        let sim = client.simulator.lock().await;
        match sim.as_ref() {
            Some(s) => {
                eprintln!("{}[OK]{} Agent ID: {}", GREEN, RESET, s.client);
                (s.client, s.seed_capability.clone())
            }
            None => {
                eprintln!("{}[ERROR]{} No simulator", RED, RESET);
                return Ok(());
            }
        }
    };

    eprintln!("{}[WAIT]{} Region handshake...", CYAN, RESET);

    match timeout(REGION_READY_TIMEOUT, async {
        while !client.region_ready.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    {
        Err(_) => {
            eprintln!("{}[TIMEOUT]{} Region handshake failed", RED, RESET);
            return Ok(());
        }
        Ok(()) => {
            eprintln!("{}[OK]{} Region ready", GREEN, RESET);
        }
    }

    eprintln!();

    // This is the same post-login capability call made by Firestorm when the
    // PreferredMaturity setting changes. The server, not the bot, decides
    // whether the requested level is available to this account.
    eprintln!(
        "{}[ACCESS]{} Setting content access to {}...",
        CYAN,
        RESET,
        requested_access.label()
    );
    match update_content_access(&seed_capability, requested_access).await {
        Ok(actual_access) if actual_access == requested_access => {
            eprintln!(
                "{}[OK]{} Content access set to {}",
                GREEN,
                RESET,
                actual_access.label()
            );
        }
        Ok(actual_access) => {
            eprintln!(
                "{}[LIMITED]{} The server applied {} instead of {}",
                YELLOW,
                RESET,
                actual_access.label(),
                requested_access.label()
            );
        }
        Err(error) => {
            eprintln!(
                "{}[FAIL]{} Could not set content access: {}",
                RED, RESET, error
            );
            return Ok(());
        }
    }

    eprintln!();

    // Scan region
    match scan_region(
        &client,
        agent_id,
        &region,
        &target_first,
        &target_last,
        &coarse_locations,
        &avatar_names,
    )
    .await
    {
        Some(uuid) => {
            eprintln!(
                "{}[SUCCESS]{} Found {} {} ({})",
                GREEN, RESET, target_first, target_last, uuid
            );
        }
        None => {
            eprintln!(
                "{}[NOT FOUND]{} {} {} not in region",
                YELLOW, RESET, target_first, target_last
            );
        }
    }

    Ok(())
}
