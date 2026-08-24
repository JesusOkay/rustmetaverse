//! Connection test for rustmetaverse.
//!
//! Performs a clean client lifecycle:
//!   1. Login via XML-RPC to the configured grid.
//!   2. Establish the UDP circuit (UseCircuitCode + CompleteAgentMovement).
//!   3. Wait for the RegionHandshake exchange.
//!   4. Print session details.
//!   5. Send a LogoutRequest and exit.
//!
//! No packet flooding, no malformed payloads — just a verify-and-disconnect.

use rustmetaverse::{GridClient, LoginParams};
use rustmetaverse_protocol::header::{Header, PacketFrequency};
use rustmetaverse_protocol::packets::{LogoutRequestAgentDataBlock, LogoutRequestPacket};
use std::io::{self, Write};
use std::time::Duration;
use tokio::time::{timeout, Instant};

/// Generate a random MD5-formatted string (32 hex chars) for the mac/id0
/// fields, so the login does not reuse the hardcoded default that many
/// bot libraries share and that grids often block.
fn random_mac_id0() -> String {
    use rand::Rng;
    let bytes: [u8; 16] = rand::thread_rng().gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Default login endpoint. Change this to your own grid's login URI
/// (e.g. http://localhost:8002 for a local OpenSim Robust instance).
const DEFAULT_LOGIN_URI: &str = "https://login.agni.lindenlab.com/cgi-bin/login.cgi";

/// How long to wait for the region handshake before giving up.
const REGION_READY_TIMEOUT: Duration = Duration::from_secs(15);

fn read_input(prompt: &str) -> String {
    eprint!("{}: ", prompt);
    io::stderr().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    eprintln!("=== rustmetaverse connection test ===");
    eprintln!();

    // --- Gather parameters -----------------------------------------------

    let first_name = read_input("First Name");
    let last_name = read_input("Last Name");
    let password = read_input("Password");
    let region = read_input("Region (or 'last')");
    let login_uri_input = read_input(&format!("Login URI [{}]", DEFAULT_LOGIN_URI));

    if first_name.is_empty() || last_name.is_empty() || password.is_empty() {
        eprintln!("[ERROR] first name, last name and password are required.");
        return Err("missing required fields".into());
    }

    let login_uri: &str = if login_uri_input.is_empty() {
        DEFAULT_LOGIN_URI
    } else {
        &login_uri_input
    };

    let start_location = if region.is_empty() || region.eq_ignore_ascii_case("last") {
        "last".to_string()
    } else {
        format!("uri:{}&128&128&30", region)
    };

    eprintln!();
    eprintln!("Connecting to: {}", login_uri);
    eprintln!("Start location: {}", start_location);
    eprintln!();

    // --- Login ------------------------------------------------------------

    let client = GridClient::new().await?;

    let params = LoginParams {
        first_name: first_name.clone(),
        last_name: last_name.clone(),
        password: password.clone(),
        start: start_location,
        mac: random_mac_id0(),
        id0: random_mac_id0(),
        ..Default::default()
    };

    let login_start = Instant::now();
    if let Err(e) = client.login(&params, login_uri).await {
        eprintln!("[FAIL] Login error: {}", e);
        return Err(e);
    }
    eprintln!(
        "[OK] Login completed in {:.2}s",
        login_start.elapsed().as_secs_f64()
    );

    // --- Wait for region handshake ---------------------------------------

    eprintln!("[..] Waiting for region handshake...");
    let wait_start = Instant::now();
    loop {
        if client
            .region_ready
            .load(std::sync::atomic::Ordering::Acquire)
        {
            break;
        }
        if wait_start.elapsed() >= REGION_READY_TIMEOUT {
            eprintln!(
                "[WARN] Region handshake did not arrive within {:.0}s. Proceeding anyway.",
                REGION_READY_TIMEOUT.as_secs_f64()
            );
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // --- Print session details -------------------------------------------

    {
        let sim_guard = client.simulator.lock().await;
        match sim_guard.as_ref() {
            Some(sim) => {
                eprintln!();
                eprintln!("--- Session details ---");
                eprintln!("  Agent ID     : {}", sim.client);
                eprintln!("  Session ID   : {}", sim.session_id);
                eprintln!("  Region ID    : {}", sim.region_id);
                eprintln!("  Region name  : {}", sim.name.trim_end_matches('\0'));
                eprintln!("  Circuit code : {}", sim.circuit_code);
                eprintln!("  Simulator    : {}", sim.ip);
                eprintln!("  Seed cap     : {}", sim.seed_capability);
                eprintln!(
                    "  Region ready : {}",
                    client
                        .region_ready
                        .load(std::sync::atomic::Ordering::Acquire)
                );
                eprintln!("-----------------------");
            }
            None => {
                eprintln!("[WARN] No simulator available after login.");
            }
        }
    }

    // --- Clean logout -----------------------------------------------------

    eprintln!();
    eprintln!("[..] Sending LogoutRequest...");

    let (agent_id, session_id) = {
        let sim_guard = client.simulator.lock().await;
        match sim_guard.as_ref() {
            Some(sim) => (sim.client, sim.session_id),
            None => {
                eprintln!("[WARN] Cannot send logout: simulator already gone.");
                return Ok(());
            }
        }
    };

    let mut logout = LogoutRequestPacket {
        header: Header {
            frequency: PacketFrequency::Low,
            id: 252,
            reliable: true,
            sequence: client.network.get_next_sequence(),
            ..Default::default()
        },
        agent_data: LogoutRequestAgentDataBlock {
            agent_i_d: agent_id,
            session_i_d: session_id,
        },
    };

    match client.network.send_packet(&mut logout).await {
        Ok(_) => eprintln!("[OK] LogoutRequest sent."),
        Err(e) => eprintln!("[WARN] Failed to send LogoutRequest: {}", e),
    }

    // Give the server a moment to process the logout before we tear down.
    let _ = timeout(
        Duration::from_millis(500),
        tokio::time::sleep(Duration::from_millis(500)),
    )
    .await;

    eprintln!("[DONE] Connection test finished.");
    Ok(())
}
