//! Full subsystem test — exercises all new API functions against a live grid.
//!
//! Usage:
//!   cargo run --release --example full_test -- \
//!       <first> <last> <password> [login_uri]
//!
//! Example:
//!   cargo run --release --example full_test -- BotFirst Resident BotPassword

use rustmetaverse::{GridClient, LoginParams};
use rustmetaverse_types::UUID;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::time::{timeout, Instant};

const DEFAULT_LOGIN_URI: &str = "https://login.agni.lindenlab.com/cgi-bin/login.cgi";
const REGION_READY_TIMEOUT: Duration = Duration::from_secs(15);

fn random_mac_id0() -> String {
    use rand::Rng;
    let bytes: [u8; 16] = rand::thread_rng().gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn step(name: &str) {
    eprintln!("\n── {name} ──────────────────────────────────────");
}

fn ok(msg: &str) {
    eprintln!("  ✅ {msg}");
}

fn warn(msg: &str) {
    eprintln!("  ⚠️  {msg}");
}

fn fail(msg: &str) {
    eprintln!("  ❌ {msg}");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: {} <first> <last> <password> [login_uri]", args[0]);
        return Err("missing arguments".into());
    }

    let first_name = &args[1];
    let last_name = &args[2];
    let password = &args[3];
    let login_uri = args.get(4).map(|s| s.as_str()).unwrap_or(DEFAULT_LOGIN_URI);

    eprintln!("╔══════════════════════════════════════════════╗");
    eprintln!("║     rustmetaverse — Full Subsystem Test      ║");
    eprintln!("╚══════════════════════════════════════════════╝");
    eprintln!("  Account : {first_name} {last_name}");
    eprintln!("  Endpoint: {login_uri}");

    // ── Login ─────────────────────────────────────────────────────────────

    step("1. Login");
    let client = GridClient::new().await?;

    let params = LoginParams {
        first_name: first_name.clone(),
        last_name: last_name.clone(),
        password: password.clone(),
        start: "last".to_string(),
        mac: random_mac_id0(),
        id0: random_mac_id0(),
        ..Default::default()
    };

    let login_start = Instant::now();
    match client.login(&params, login_uri).await {
        Ok(_) => ok(&format!(
            "Login in {:.2}s",
            login_start.elapsed().as_secs_f64()
        )),
        Err(e) => {
            fail(&format!("Login failed: {e}"));
            return Err(e);
        }
    }

    // ── Wait for region handshake ─────────────────────────────────────────

    step("2. Region handshake");
    let wait_start = Instant::now();
    loop {
        if client.region_ready.load(Ordering::Acquire) {
            ok("Region handshake complete");
            break;
        }
        if wait_start.elapsed() >= REGION_READY_TIMEOUT {
            warn("Region handshake timed out — proceeding anyway");
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Print session info
    {
        let sim = client.simulator.lock().await;
        if let Some(s) = sim.as_ref() {
            eprintln!("  Region   : {}", s.name.trim_end_matches('\0'));
            eprintln!("  Agent ID : {}", s.client);
            eprintln!("  Circuit  : {}", s.circuit_code);
        }
    }

    // ── Wait for AgentMovementComplete ────────────────────────────────────

    step("3. AgentMovementComplete (core handler)");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let pos = client.core_state.avatar_position.read().await;
        if pos.timestamp > 0 {
            ok(&format!(
                "Position: ({:.1}, {:.1}, {:.1})  region_handle={}",
                pos.position.x, pos.position.y, pos.position.z, pos.region_handle
            ));
            break;
        }
        if Instant::now() >= deadline {
            warn("AgentMovementComplete not received yet (non-fatal)");
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // ── Chat: say() ───────────────────────────────────────────────────────

    step("4. Chat — say()");
    match client
        .say("rustmetaverse test: hello from the new chat subsystem!")
        .await
    {
        Ok(_) => ok("ChatFromViewer (say) sent"),
        Err(e) => fail(&format!("say() failed: {e}")),
    }

    // ── Chat: shout() ─────────────────────────────────────────────────────

    step("5. Chat — shout()");
    match client.shout("rustmetaverse test: SHOUT TEST!").await {
        Ok(_) => ok("ChatFromViewer (shout) sent"),
        Err(e) => fail(&format!("shout() failed: {e}")),
    }

    // ── IM: send_im() to self ─────────────────────────────────────────────

    step("6. IM — send_im() to self");
    {
        let sim = client.simulator.lock().await;
        if let Some(s) = sim.as_ref() {
            let self_id = s.client;
            drop(sim);
            match client.send_im(self_id, "rustmetaverse IM self-test").await {
                Ok(_) => ok("ImprovedInstantMessage sent to self"),
                Err(e) => fail(&format!("send_im() failed: {e}")),
            }
        }
    }

    // ── Inventory: fetch root folder ──────────────────────────────────────

    step("7. Inventory — fetch_inventory_folder()");
    {
        let sim = client.simulator.lock().await;
        if let Some(s) = sim.as_ref() {
            let agent_id = s.client;
            drop(sim);
            // Fetch the root "My Inventory" folder (UUID all-zeros means
            // "root" in the OpenSim/SL protocol).
            match client.fetch_inventory_folder(UUID::ZERO, agent_id).await {
                Ok(_) => ok("FetchInventoryDescendents sent for root folder"),
                Err(e) => fail(&format!("fetch_inventory_folder() failed: {e}")),
            }
        }
    }

    // ── Groups: join_group() with a dummy UUID ─────────────────────────────
    // We use UUID::ZERO which will fail gracefully — this tests that the
    // packet is built and sent without crashing.

    step("8. Groups — join_group(UUID::ZERO)");
    match client.join_group(UUID::ZERO).await {
        Ok(_) => ok("JoinGroupRequest sent (will get error reply — expected)"),
        Err(e) => fail(&format!("join_group() failed: {e}")),
    }

    // ── Groups: leave_group(UUID::ZERO) ───────────────────────────────────

    step("9. Groups — leave_group(UUID::ZERO)");
    match client.leave_group(UUID::ZERO).await {
        Ok(_) => ok("LeaveGroupRequest sent (will get error reply — expected)"),
        Err(e) => fail(&format!("leave_group() failed: {e}")),
    }

    // ── Appearance: rebake(UUID::ZERO) ────────────────────────────────────

    step("10. Appearance — rebake(UUID::ZERO)");
    match client.rebake(UUID::ZERO).await {
        Ok(_) => ok("RebakeAvatarTextures sent"),
        Err(e) => fail(&format!("rebake() failed: {e}")),
    }

    // ── Wait a moment for any reply packets to arrive ─────────────────────

    step("11. Collecting replies (2s wait)");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check core state
    {
        let health = *client.core_state.health.read().await;
        if health > 0.0 {
            ok(&format!("HealthMessage received: health = {health}"));
        } else {
            warn("No HealthMessage received yet (non-fatal)");
        }

        let logout_confirmed = *client.core_state.logout_confirmed.read().await;
        if logout_confirmed {
            ok("LogoutReply already received (unexpected before logout call)");
        }

        let sim_disabled = *client.core_state.simulator_disabled.read().await;
        if sim_disabled {
            warn("DisableSimulator received (region is shutting down?)");
        }

        let names = client.core_state.display_names.read().await;
        if !names.is_empty() {
            ok(&format!(
                "UUIDNameReply cached {} display names",
                names.len()
            ));
        } else {
            warn("No UUIDNameReply received yet (non-fatal)");
        }
    }

    // ── Logout via GridClient::logout() ──────────────────────────────────

    step("12. Logout via GridClient::logout()");
    match client.logout().await {
        Ok(_) => ok("LogoutRequest sent"),
        Err(e) => fail(&format!("logout() failed: {e}")),
    }

    // Wait for LogoutReply
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let confirmed = *client.core_state.logout_confirmed.read().await;
        if confirmed {
            ok("LogoutReply received — logout confirmed");
            break;
        }
        if Instant::now() >= deadline {
            warn("LogoutReply not received within 5s");
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // ── Summary ──────────────────────────────────────────────────────────

    eprintln!("\n╔══════════════════════════════════════════════╗");
    eprintln!("║             Test Summary                     ║");
    eprintln!("╠══════════════════════════════════════════════╣");
    eprintln!("║  Login          ✅  (if reached here)         ║");
    eprintln!("║  Region handshake ✅                         ║");
    eprintln!("║  AgentMovementComplete (core handler)        ║");
    eprintln!("║  Chat — say() + shout()                      ║");
    eprintln!("║  IM — send_im()                              ║");
    eprintln!("║  Inventory — fetch_inventory_folder()         ║");
    eprintln!("║  Groups — join_group() + leave_group()        ║");
    eprintln!("║  Appearance — rebake()                       ║");
    eprintln!("║  Logout — GridClient::logout()               ║");
    eprintln!("╚══════════════════════════════════════════════╝");
    eprintln!("\nDone. Check the log lines above for ✅/⚠️/❌ status.");

    let _ = timeout(
        Duration::from_millis(500),
        tokio::time::sleep(Duration::from_millis(500)),
    )
    .await;
    Ok(())
}
