//! A bot over WebTransport and a bot over ENet against a running stack. Run by hand:
//! `CE2E_STACK=<stack file> cargo test -p catalyst-comms-e2e --test live_webtransport -- --ignored`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use catalyst_comms_e2e::engine::{Ctx, Transport, expect_or};
use catalyst_comms_e2e::pulse::{ConnectOptions, PulseBot};
use catalyst_comms_e2e::stack::Stack;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs a running stack: CE2E_STACK names its stack file"]
async fn a_webtransport_bot_and_an_enet_bot_see_each_other_on_the_live_stack() {
    let path = PathBuf::from(std::env::var("CE2E_STACK").expect("CE2E_STACK names the stack file"));
    let stack = Arc::new(Stack::load(&path).unwrap());
    let ctx = |transport| {
        Ctx::new_on(
            stack.clone(),
            &format!("live-{}", std::process::id()),
            "live-webtransport",
            0,
            1,
            std::env::temp_dir().join("comms-e2e-tests"),
            transport,
        )
    };
    let (over_enet, over_webtransport) = (ctx(Transport::Enet), ctx(Transport::WebTransport));
    let realm = over_enet.realm("live");
    let (enet_id, wt_id) = (over_enet.identity(0), over_enet.identity(1));

    let enet_bot = PulseBot::enter(&over_enet, "enet-bot", &enet_id)
        .await
        .unwrap();
    enet_bot.teleport(&realm, [8.0, 0.0, 8.0]).unwrap();
    let wt_bot = PulseBot::dial(
        &over_webtransport,
        "webtransport-bot",
        ConnectOptions::default(),
    )
    .await
    .unwrap();
    wt_bot.accepted(&over_webtransport, &wt_id).await.unwrap();
    wt_bot.teleport(&realm, [9.0, 0.0, 9.0]).unwrap();

    expect_or(
        &over_enet,
        "the ENet bot sees the WebTransport bot",
        Duration::from_secs(10),
        || {
            enet_bot.alive()?;
            enet_bot
                .view()
                .of(&wt_id.address())
                .map(|_| ())
                .ok_or_else(|| "not yet".to_owned())
        },
    )
    .await
    .unwrap();
    expect_or(
        &over_enet,
        "the WebTransport bot sees the ENet bot",
        Duration::from_secs(10),
        || {
            wt_bot.alive()?;
            wt_bot
                .view()
                .of(&enet_id.address())
                .map(|_| ())
                .ok_or_else(|| "not yet".to_owned())
        },
    )
    .await
    .unwrap();
    wt_bot.close();
    enet_bot.close();
}
