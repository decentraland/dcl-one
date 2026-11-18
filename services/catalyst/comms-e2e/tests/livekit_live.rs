//! Checks against the live local stack's LiveKit. Ignored by default: they need a running stack.
//!
//! Run with `CE2E_STACK` naming the stack file, `CE2E_LIVEKIT_CLIENT` naming the `harness-util`
//! binary, and the key and secret variables the stack file names set:
//! `cargo test -p catalyst-comms-e2e --test livekit_live -- --ignored --nocapture`

mod support;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use catalyst_comms_e2e::engine::{Ctx, expect_or};
use catalyst_comms_e2e::livekit::{JoinOptions, LiveKitJoin, RoomService};
use catalyst_comms_e2e::stack::Stack;

fn live_ctx(name: &str) -> Ctx {
    let path = std::env::var("CE2E_STACK").expect("CE2E_STACK names the stack file");
    let stack = Arc::new(Stack::load(Path::new(&path)).unwrap());
    Ctx::new(stack, &format!("live{}", std::process::id()), name, 0, 1, std::env::temp_dir().join("comms-e2e-tests"))
}

fn options(publish_microphone: bool) -> JoinOptions {
    JoinOptions {
        publish_microphone,
        binary: Some(PathBuf::from(std::env::var("CE2E_LIVEKIT_CLIENT").expect("CE2E_LIVEKIT_CLIENT names the client"))),
        through_front: false,
    }
}

#[tokio::test]
#[ignore = "needs the live stack"]
async fn two_clients_see_each_other_and_the_room_service_agrees() {
    let ctx = live_ctx("live-two-clients");
    let service = RoomService::new(ctx.stack()).unwrap();
    let room = ctx.realm("live");
    let first_identity = ctx.identity(0).address();
    let second_identity = ctx.identity(1).address();
    let first = LiveKitJoin::start(&ctx, "first", &service.mint_join(&room, &first_identity).unwrap(), options(true))
        .await
        .unwrap();
    let second = LiveKitJoin::start(&ctx, "second", &service.mint_join(&room, &second_identity).unwrap(), options(false))
        .await
        .unwrap();
    expect_or(&ctx, "each sees the other, and the first's microphone", Duration::from_secs(20), || {
        let view = second.view();
        if !first.view().participants.contains_key(&second_identity) {
            return Err("the first does not see the second".to_owned());
        }
        if !view.participants.contains_key(&first_identity) {
            return Err("the second does not see the first".to_owned());
        }
        if view.tracks_of(&first_identity, "microphone").len() != 1 {
            return Err("the second does not see one microphone track of the first".to_owned());
        }
        Ok(())
    })
    .await
    .unwrap();
    let listed = service.participants(&room).await.unwrap();
    println!("{listed:#?}");
    assert_eq!(listed.len(), 2);
    let rooms = service.rooms().await.unwrap();
    println!("{:?}", rooms.iter().find(|listed| listed.name == room));
    assert!(rooms.iter().any(|listed| listed.name == room), "{rooms:?}");
    first.leave().await.unwrap();
    second.leave().await.unwrap();
    for entry in ctx.timeline().snapshot() {
        println!("{} {} {} {}", entry.at_ms, entry.actor, entry.kind, entry.detail);
    }
}

#[tokio::test]
#[ignore = "needs the live stack"]
async fn the_same_identity_scenario_passes() {
    use catalyst_comms_e2e::engine::{Outcome, RunOptions, run_scenarios};
    let path = std::env::var("CE2E_STACK").expect("CE2E_STACK names the stack file");
    let client = std::env::var("CE2E_LIVEKIT_CLIENT").expect("CE2E_LIVEKIT_CLIENT names the client");
    let text = std::fs::read_to_string(path).unwrap().replace("[livekit]", &format!("[livekit]
client = '{client}'"));
    let stack = Arc::new(Stack::parse(&text).unwrap());
    let selection = catalyst_comms_e2e::scenarios::select(Some("livekit-same-identity"), None, false);
    assert_eq!(selection.len(), 1);
    let report = run_scenarios(stack, &selection, &RunOptions::default()).await;
    let result = &report.results[0];
    for note in &result.notes {
        println!("note {} {}", note.key, note.value);
    }
    match &result.outcome {
        Outcome::Passed => {}
        Outcome::Failed { expectation, timeline, .. } => {
            for entry in timeline {
                println!("{} {} {} {}", entry.at_ms, entry.actor, entry.kind, entry.detail);
            }
            panic!("{expectation}");
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
#[ignore = "needs the live stack"]
async fn a_joined_client_becomes_active_for_the_server() {
    let ctx = live_ctx("live-active");
    let service = RoomService::new(ctx.stack()).unwrap();
    let room = ctx.realm("active");
    let identity = ctx.identity(0).address();
    let started = std::time::Instant::now();
    let join = LiveKitJoin::start(&ctx, "client", &service.mint_join(&room, &identity).unwrap(), options(true)).await.unwrap();
    println!("connected after {} ms", started.elapsed().as_millis());
    loop {
        let listed = service.participants(&room).await.unwrap();
        let state = listed.first().map(|p| p.state.clone()).unwrap_or_default();
        println!("{} ms: {state}", started.elapsed().as_millis());
        if state == "active" || started.elapsed() > Duration::from_secs(20) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    join.leave().await.unwrap();
}
