//! `/ban_block_state` and the "Bans and blocks" debug section.

use alloy_core::primitives::Address;
use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::util::AsH160;
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use serde_json::{json, Map, Value};
use wallet::Wallet;

use crate::{BanBlock, BlockDirection, Blocks};

pub(crate) fn register(app: &mut App) {
    app.add_console_command::<BanBlockStateCommand, _>(ban_block_state)
        .add_debug_section(DebugSection {
            title: "Bans and blocks",
            query: "/ban_block_state",
            fields: &[
                ("Blocks", "blocks.state"),
                ("Blocked", "blocks.blocked"),
                ("Blocked by", "blocks.blockedBy"),
                ("Lookup: blocked either way", "lookup.isBlockedEitherWay"),
                ("Lookup: direction", "lookup.direction"),
            ],
            actions: &[],
            inputs: &[DebugInput {
                label: "Look up address",
                field: "lookup.address",
                command: "/ban_block_state {value}",
                kind: DebugInputKind::Text,
            }],
        });
}

/// The ban and block status, as JSON; with an address, also that address's standing.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/ban_block_state")]
struct BanBlockStateCommand {
    address: Option<String>,
}

fn ban_block_state(
    mut input: ConsoleCommand<BanBlockStateCommand>,
    ban_block: BanBlock,
    wallet: Res<Wallet>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let lookup = match command.address.as_deref() {
        None => None,
        Some(raw) => match raw.as_h160() {
            Some(address) => Some(address),
            None => {
                input.reply_failed("invalid address");
                return;
            }
        },
    };
    input.reply_ok(snapshot(&ban_block, wallet.address(), lookup).to_string());
}

fn hex(address: Address) -> String {
    format!("{address:#x}")
}

fn addresses(set: &bevy::platform::collections::HashSet<Address>) -> Value {
    let mut list: Vec<String> = set.iter().copied().map(hex).collect();
    list.sort();
    json!(list)
}

/// Every `null` reading has its reason in `unavailable`, keyed by the field path.
pub(crate) fn snapshot(ban_block: &BanBlock, own: Option<Address>, lookup: Option<Address>) -> Value {
    let mut unavailable = Map::new();
    let blocks = ban_block.blocks();
    let blocks_json = match blocks {
        Blocks::Live {
            blocked,
            blocked_by,
        } => json!({
            "state": "live",
            "reason": null,
            "blocked": addresses(blocked),
            "blockedBy": addresses(blocked_by),
        }),
        Blocks::Unavailable { reason } => {
            let why = match reason {
                crate::BlocksUnavailableReason::SocialFeatureOff => "social feature off",
                crate::BlocksUnavailableReason::NotSignedIn => "not signed in",
                crate::BlocksUnavailableReason::SocialNotConnected => "social not connected",
            };
            unavailable.insert("blocks.blocked".into(), json!(why));
            unavailable.insert("blocks.blockedBy".into(), json!(why));
            json!({
                "state": "unavailable",
                "reason": reason,
                "blocked": [],
                "blockedBy": [],
            })
        }
    };
    let lookup_json = match lookup {
        Some(address) => {
            let direction = blocks.block_direction(address);
            json!({
                "address": hex(address),
                "isBlockedEitherWay": direction != BlockDirection::None,
                "direction": direction,
            })
        }
        None => {
            unavailable.insert("lookup".into(), json!("no address given"));
            Value::Null
        }
    };
    json!({
        "address": own.map(hex),
        "blocks": blocks_json,
        "lookup": lookup_json,
        "unavailable": unavailable,
    })
}
