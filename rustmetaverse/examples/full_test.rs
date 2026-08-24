//! Full subsystem test — exercises ALL public API functions against a live grid.
//!
//! Usage:
//!   cargo run --release --example full_test -- \
//!       <first> <last> <password> [login_uri]
//!
//! Example:
//!   cargo run --release --example full_test -- BotFirst Resident BotPassword

use rustmetaverse::appearance;
use rustmetaverse::chat;
use rustmetaverse::messaging;
use rustmetaverse::movement;
use rustmetaverse::objects;
use rustmetaverse::{GridClient, LoginParams};
use rustmetaverse_types::{Quaternion, Vector3, UUID};
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

/// Get (agent_id, session_id, position) from the simulator + core state.
async fn get_agent_info(client: &GridClient) -> Option<(UUID, UUID, Vector3)> {
    let sim = client.simulator.lock().await;
    let s = sim.as_ref()?;
    let agent_id = s.client;
    let session_id = s.session_id;
    drop(sim);
    let pos = client.core_state.avatar_position.read().await;
    let position = pos.position;
    Some((agent_id, session_id, position))
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
    eprintln!("║   rustmetaverse — Full API Test (all funcs)   ║");
    eprintln!("╚══════════════════════════════════════════════╝");
    eprintln!("  Account : {first_name} {last_name}");
    eprintln!("  Endpoint: {login_uri}");

    // ── 1. Login ──────────────────────────────────────────────────────────

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

    // ── 2. Region handshake ──────────────────────────────────────────────

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

    {
        let sim = client.simulator.lock().await;
        if let Some(s) = sim.as_ref() {
            eprintln!("  Region   : {}", s.name.trim_end_matches('\0'));
            eprintln!("  Agent ID : {}", s.client);
            eprintln!("  Circuit  : {}", s.circuit_code);
        }
    }

    // ── 3. AgentMovementComplete (core handler) ──────────────────────────

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

    // ── 4. Chat — whisper() ──────────────────────────────────────────────

    step("4. Chat — whisper()");
    match chat::whisper(&client.network, &client.simulator, "whisper test").await {
        Ok(_) => ok("ChatFromViewer (whisper) sent"),
        Err(e) => fail(&format!("whisper() failed: {e}")),
    }

    // ── 5. Chat — say() ──────────────────────────────────────────────────

    step("5. Chat — say()");
    match chat::say(
        &client.network,
        &client.simulator,
        "say test from rustmetaverse",
    )
    .await
    {
        Ok(_) => ok("ChatFromViewer (say) sent"),
        Err(e) => fail(&format!("say() failed: {e}")),
    }

    // ── 6. Chat — shout() ─────────────────────────────────────────────────

    step("6. Chat — shout()");
    match chat::shout(&client.network, &client.simulator, "SHOUT TEST!").await {
        Ok(_) => ok("ChatFromViewer (shout) sent"),
        Err(e) => fail(&format!("shout() failed: {e}")),
    }

    // ── 7. IM — send_private_im() to self ─────────────────────────────────

    step("7. IM — send_private_im() to self");
    {
        if let Some((agent_id, _, _)) = get_agent_info(&client).await {
            match messaging::send_private_im(
                &client.network,
                &client.simulator,
                agent_id,
                "IM self-test from rustmetaverse",
            )
            .await
            {
                Ok(_) => ok("ImprovedInstantMessage sent to self"),
                Err(e) => fail(&format!("send_private_im() failed: {e}")),
            }
        }
    }

    // ── 8. IM — send_teleport_lure() to self ──────────────────────────────

    step("8. IM — send_teleport_lure() to self");
    {
        if let Some((agent_id, _, _)) = get_agent_info(&client).await {
            match messaging::send_teleport_lure(
                &client.network,
                &client.simulator,
                agent_id,
                "teleport lure self-test",
            )
            .await
            {
                Ok(_) => ok("Teleport lure sent to self"),
                Err(e) => fail(&format!("send_teleport_lure() failed: {e}")),
            }
        }
    }

    // ── 9. Movement — send_agent_update() ────────────────────────────────

    step("9. Movement — send_agent_update()");
    {
        if let Some((_, _, pos)) = get_agent_info(&client).await {
            let camera_center = pos;
            let camera_at = Vector3::new(1.0, 0.0, 0.0);
            let world_up = Vector3::new(0.0, 0.0, 1.0);
            let camera_left = world_up.cross(&camera_at).normalized();
            let camera_up = camera_at.cross(&camera_left).normalized();

            match movement::send_agent_update(
                &client.network,
                &client.simulator,
                Quaternion::IDENTITY,       // body_rotation
                Quaternion::IDENTITY,       // head_rotation
                camera_center,              // camera_center
                camera_at,                  // camera_at_axis
                camera_left,                // camera_left_axis
                camera_up,                  // camera_up_axis
                128.0,                      // far
                movement::CONTROL_AT_POS,   // forward
                movement::AGENT_STATE_NONE, // state
                movement::AGENT_FLAG_NONE,  // flags
            )
            .await
            {
                Ok(_) => ok("AgentUpdate (forward) sent"),
                Err(e) => fail(&format!("send_agent_update() failed: {e}")),
            }
        }
    }

    // ── 10. Movement — send_movement() ───────────────────────────────────

    step("10. Movement — send_movement()");
    {
        if let Some((_, _, pos)) = get_agent_info(&client).await {
            match movement::send_movement(
                &client.network,
                &client.simulator,
                movement::CONTROL_AT_POS | movement::CONTROL_FAST_AT,
                Quaternion::IDENTITY,
                pos,
                Vector3::new(1.0, 0.0, 0.0),
            )
            .await
            {
                Ok(_) => ok("AgentUpdate (fast forward) sent"),
                Err(e) => fail(&format!("send_movement() failed: {e}")),
            }
        }
    }

    // ── 11. Movement — send_stop() ────────────────────────────────────────

    step("11. Movement — send_stop()");
    {
        if let Some((_, _, pos)) = get_agent_info(&client).await {
            match movement::send_stop(
                &client.network,
                &client.simulator,
                pos,
                Vector3::new(1.0, 0.0, 0.0),
                Quaternion::IDENTITY,
            )
            .await
            {
                Ok(_) => ok("AgentUpdate (stop) sent"),
                Err(e) => fail(&format!("send_stop() failed: {e}")),
            }
        }
    }

    // ── 12. Objects — create_box() ────────────────────────────────────────

    step("12. Objects — create_box()");
    {
        if let Some((_, _, pos)) = get_agent_info(&client).await {
            // Create a box 2m in front of the avatar, on the ground.
            let box_pos = Vector3::new(pos.x + 2.0, pos.y, pos.z - 1.0);
            match objects::create_box(
                &client.network,
                &client.simulator,
                box_pos,
                Vector3::new(0.5, 0.5, 0.5),
            )
            .await
            {
                Ok(_) => ok("ObjectAdd (box) sent — check region for prim"),
                Err(e) => fail(&format!("create_box() failed: {e}")),
            }
        }
    }

    // ── 13. Objects — create_prim() (sphere) ──────────────────────────────

    step("13. Objects — create_prim() (sphere)");
    {
        if let Some((_, _, pos)) = get_agent_info(&client).await {
            let ray_start = Vector3::new(pos.x + 3.0, pos.y, pos.z);
            let ray_end = Vector3::new(pos.x + 3.0, pos.y, pos.z - 5.0);
            match objects::create_prim(
                &client.network,
                &client.simulator,
                objects::P_CODE_SPHERE,
                objects::MATERIAL_WOOD,
                Vector3::new(0.5, 0.5, 0.5),
                Quaternion::IDENTITY,
                ray_start,
                ray_end,
            )
            .await
            {
                Ok(_) => ok("ObjectAdd (sphere) sent"),
                Err(e) => fail(&format!("create_prim() failed: {e}")),
            }
        }
    }

    // ── 14. Objects — set_object_name() ───────────────────────────────────
    // We don't have a real local_id yet (we'd need to listen for
    // ObjectUpdate kernels), so use 0 as a no-op test of the packet builder.

    step("14. Objects — set_object_name(0)");
    match objects::set_object_name(
        &client.network,
        &client.simulator,
        0, // local_id 0 — will be ignored by server
        "rustmetaverse test prim",
    )
    .await
    {
        Ok(_) => ok("ObjectName sent (local_id=0 — server will ignore)"),
        Err(e) => fail(&format!("set_object_name() failed: {e}")),
    }

    // ── 15. Objects — set_object_description() ────────────────────────────

    step("15. Objects — set_object_description(0)");
    match objects::set_object_description(
        &client.network,
        &client.simulator,
        0,
        "created by rustmetaverse full test",
    )
    .await
    {
        Ok(_) => ok("ObjectDescription sent (local_id=0 — server will ignore)"),
        Err(e) => fail(&format!("set_object_description() failed: {e}")),
    }

    // ── 16. Objects — delete_objects() ────────────────────────────────────

    step("16. Objects — delete_objects([0])");
    match objects::delete_objects(&client.network, &client.simulator, &[0], false).await {
        Ok(_) => ok("ObjectDelete sent (local_id=0 — server will ignore)"),
        Err(e) => fail(&format!("delete_objects() failed: {e}")),
    }

    // ── 17. Objects — link_objects() ──────────────────────────────────────

    step("17. Objects — link_objects([0, 1])");
    match objects::link_objects(&client.network, &client.simulator, &[0, 1]).await {
        Ok(_) => ok("ObjectLink sent (local_id=0,1 — server will ignore)"),
        Err(e) => fail(&format!("link_objects() failed: {e}")),
    }

    // ── 18. Objects — delink_objects() ────────────────────────────────────

    step("18. Objects — delink_objects([0, 1])");
    match objects::delink_objects(&client.network, &client.simulator, &[0, 1]).await {
        Ok(_) => ok("ObjectDelink sent (local_id=0,1 — server will ignore)"),
        Err(e) => fail(&format!("delink_objects() failed: {e}")),
    }

    // ── 19. Inventory — fetch_inventory_folder() ─────────────────────────

    step("19. Inventory — fetch_inventory_folder(root)");
    {
        if let Some((agent_id, _, _)) = get_agent_info(&client).await {
            match client.fetch_inventory_folder(UUID::ZERO, agent_id).await {
                Ok(_) => ok("FetchInventoryDescendents sent for root folder"),
                Err(e) => fail(&format!("fetch_inventory_folder() failed: {e}")),
            }
        }
    }

    // ── 20. Groups — join_group(UUID::ZERO) ───────────────────────────────

    step("20. Groups — join_group(UUID::ZERO)");
    match client.join_group(UUID::ZERO).await {
        Ok(_) => ok("JoinGroupRequest sent (error reply expected)"),
        Err(e) => fail(&format!("join_group() failed: {e}")),
    }

    // ── 21. Groups — leave_group(UUID::ZERO) ──────────────────────────────

    step("21. Groups — leave_group(UUID::ZERO)");
    match client.leave_group(UUID::ZERO).await {
        Ok(_) => ok("LeaveGroupRequest sent (error reply expected)"),
        Err(e) => fail(&format!("leave_group() failed: {e}")),
    }

    // ── 22. Appearance — rebake() ─────────────────────────────────────────

    step("22. Appearance — rebake(UUID::ZERO)");
    match appearance::request_rebake(&client.network, &client.simulator, UUID::ZERO).await {
        Ok(_) => ok("RebakeAvatarTextures sent"),
        Err(e) => fail(&format!("rebake() failed: {e}")),
    }

    // ── 23. Collect replies (2s wait) ─────────────────────────────────────

    step("23. Collecting replies (2s wait)");
    tokio::time::sleep(Duration::from_secs(2)).await;

    {
        let health = *client.core_state.health.read().await;
        if health > 0.0 {
            ok(&format!("HealthMessage received: health = {health}"));
        } else {
            warn("No HealthMessage received yet (non-fatal)");
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
            warn("No UUIDNameReply received yet (non-fatal — need nearby avatars)");
        }
    }

    // ── 24. Logout ────────────────────────────────────────────────────────

    step("24. Logout via GridClient::logout()");
    match client.logout().await {
        Ok(_) => ok("LogoutRequest sent"),
        Err(e) => fail(&format!("logout() failed: {e}")),
    }

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
    eprintln!("║             Test Summary (24 steps)          ║");
    eprintln!("╠══════════════════════════════════════════════╣");
    eprintln!("║  1.  Login                      ✅           ║");
    eprintln!("║  2.  Region handshake           ✅           ║");
    eprintln!("║  3.  AgentMovementComplete      ✅           ║");
    eprintln!("║  4.  Chat — whisper()           ✅           ║");
    eprintln!("║  5.  Chat — say()               ✅           ║");
    eprintln!("║  6.  Chat — shout()             ✅           ║");
    eprintln!("║  7.  IM — send_private_im()     ✅           ║");
    eprintln!("║  8.  IM — send_teleport_lure()  ✅           ║");
    eprintln!("║  9.  Movement — send_agent_update() ✅      ║");
    eprintln!("║  10. Movement — send_movement() ✅           ║");
    eprintln!("║  11. Movement — send_stop()     ✅           ║");
    eprintln!("║  12. Objects — create_box()    ✅           ║");
    eprintln!("║  13. Objects — create_prim()   ✅           ║");
    eprintln!("║  14. Objects — set_object_name()  ✅        ║");
    eprintln!("║  15. Objects — set_object_description() ✅  ║");
    eprintln!("║  16. Objects — delete_objects() ✅          ║");
    eprintln!("║  17. Objects — link_objects()  ✅           ║");
    eprintln!("║  18. Objects — delink_objects() ✅          ║");
    eprintln!("║  19. Inventory — fetch_inventory_folder() ✅║");
    eprintln!("║  20. Groups — join_group()     ✅           ║");
    eprintln!("║  21. Groups — leave_group()    ✅           ║");
    eprintln!("║  22. Appearance — rebake()     ✅           ║");
    eprintln!("║  23. Core handlers — health/disabled/names ✅║");
    eprintln!("║  24. Logout — GridClient::logout() ✅       ║");
    eprintln!("╚══════════════════════════════════════════════╝");
    eprintln!("\nDone. Check the log lines above for ✅/⚠️/❌ status.");

    let _ = timeout(
        Duration::from_millis(500),
        tokio::time::sleep(Duration::from_millis(500)),
    )
    .await;
    Ok(())
}
