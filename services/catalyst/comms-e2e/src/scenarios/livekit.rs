//! Scenarios that need LiveKit alone: clients that join a room without the gatekeeper.

use std::time::Duration;

use anyhow::Result;

use crate::engine::{Ctx, Needs, Scenario, ScenarioFuture, expect_async, expect_or};
use crate::livekit::{JoinOptions, LiveKitJoin, RoomService};

/// The scenarios of this group, in the order a run takes them.
pub fn all() -> Vec<&'static Scenario> {
    vec![&SAME_IDENTITY]
}

const SOON: Duration = Duration::from_secs(20);

static SAME_IDENTITY: Scenario = Scenario {
    name: "livekit-same-identity",
    tags: &["connections", "livekit"],
    guards: &["dcl-one #1205"],
    summary: "A second join with the same identity replaces the first: the room keeps one participant, the first client is told why, the others see one leave and one join",
    needs: Needs {
        api: false,
        webtransport: false,
        pulse_metrics: false,
        livekit: true,
        moderator: false,
        explorer: false,
    },
    hostile: false,
    default_repeat: 1,
    run: |ctx| -> ScenarioFuture { Box::pin(same_identity(ctx)) },
};

async fn same_identity(ctx: Ctx) -> Result<()> {
    let service = RoomService::new(ctx.stack())?;
    let room = ctx.realm("livekit");
    let player = ctx.identity(0).address();
    let watcher = ctx.identity(1).address();

    let first = LiveKitJoin::start(
        &ctx,
        "first",
        &service.mint_join(&room, &player)?,
        JoinOptions::default(),
    )
    .await?;
    let observer = LiveKitJoin::start(
        &ctx,
        "observer",
        &service.mint_join(&room, &watcher)?,
        JoinOptions::default(),
    )
    .await?;
    let first_sid = first.view().sid.unwrap_or_default();
    expect_or(&ctx, "the observer sees the first client", SOON, || {
        observer.alive()?;
        match observer.view().participants.get(&player) {
            Some(sid) if *sid == first_sid => Ok(()),
            Some(sid) => Err(format!("it sees session {sid}, not {first_sid}")),
            None => Err("it sees nobody with that identity".to_owned()),
        }
    })
    .await?;

    let second = LiveKitJoin::start(
        &ctx,
        "second",
        &service.mint_join(&room, &player)?,
        JoinOptions::default(),
    )
    .await?;
    let second_sid = second.view().sid.unwrap_or_default();
    ctx.note(
        "sessions",
        serde_json::json!({ "first": first_sid, "second": second_sid }),
    );

    expect_or(
        &ctx,
        "the first client is told that its identity joined again",
        SOON,
        || match first.view().disconnect_reason {
            Some(reason) if reason == "DUPLICATE_IDENTITY" => Ok(()),
            Some(reason) => Err(format!(
                "fatal: it was disconnected for another reason: {reason}"
            )),
            None => Err("it has not been disconnected".to_owned()),
        },
    )
    .await?;

    expect_async(
        &ctx,
        "the room service lists exactly one participant with that identity, the second session",
        SOON,
        Duration::from_millis(200),
        || async {
            let listed = service
                .participants(&room)
                .await
                .map_err(|error| format!("{error:#}"))?;
            let of_player: Vec<_> = listed.iter().filter(|p| p.identity == player).collect();
            match of_player.as_slice() {
                [only] if only.sid == second_sid => Ok(()),
                [only] => Err(format!("it lists session {}, not {second_sid}", only.sid)),
                others => Err(format!(
                    "it lists {} participants with that identity",
                    others.len()
                )),
            }
        },
    )
    .await?;

    expect_or(
        &ctx,
        "the observer saw one leave and one join, and holds the second session",
        SOON,
        || {
            observer.alive()?;
            let view = observer.view();
            let (joins, leaves) = view.arrivals_of(&player);
            // the first join and the second, and the one departure between them
            if (joins, leaves) != (2, 1) {
                return Err(format!("it saw {joins} joins and {leaves} leaves"));
            }
            match view.participants.get(&player) {
                Some(sid) if *sid == second_sid => Ok(()),
                Some(sid) => Err(format!("it holds session {sid}")),
                None => Err("it holds nobody with that identity".to_owned()),
            }
        },
    )
    .await?;
    let order: Vec<String> = observer
        .view()
        .arrivals
        .iter()
        .filter(|arrival| arrival.identity == player)
        .map(|arrival| {
            format!(
                "{} {}",
                if arrival.joined { "join" } else { "leave" },
                arrival.sid
            )
        })
        .collect();
    ctx.note("observer saw, in order", order);

    second
        .alive()
        .map_err(|reason| anyhow::anyhow!("the second client: {reason}"))?;
    observer.leave().await?;
    second.leave().await?;
    Ok(())
}
