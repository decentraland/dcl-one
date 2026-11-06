//! Local DM and community chat history: every DM or community message sent or received, and
//! every reaction to one, is stored per account and read back per conversation (a partner's
//! wallet or `community:<id>`), reactions folded onto their messages.
//! Storage is the platform crate's (encrypted files on native, IndexedDB on web). Nothing here
//! talks to a server, and which conversations the HUD shows is the HUD's business.
//!
//! Store operations run one at a time, in order: a history read answers with every DM that was
//! emitted before it, including the one that made the HUD ask.

use std::collections::{HashMap, VecDeque};

use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use bevy_console::ConsoleCommand;
use common::{
    rpc::RpcResultSender,
    structs::PrimaryUser,
    util::{AsH160, TaskExt},
};
use comms::{
    chat_reaction::{chat_message_id, ChatReactionEvent},
    community_chat::{channel_for, PrivateChatTarget},
    global_crdt::ChatEvent,
    private_chat::{CommunityChatReceived, PrivateChatReceived},
};
use console::PendingConsoleResponses;
use platform::{DmHistoryEntry, DmHistoryReaction};
use social::SocialClient;
use system_bridge::{DmHistoryEntryData, DmReactionData, SystemApi};
use wallet::Wallet;

use super::dm_state::blocked_by_me;

enum DmStoreOp {
    Append {
        account: String,
        partner: String,
        entry: DmHistoryEntry,
    },
    Read {
        account: String,
        partner: String,
        sender: RpcResultSender<Vec<DmHistoryEntryData>>,
    },
    Delete {
        account: String,
        partner: String,
    },
    /// Keeps the last `keep` messages of a conversation and the reactions to them, within the
    /// last `keep_records` records.
    Trim {
        account: String,
        partner: String,
        keep: usize,
        keep_records: usize,
    },
}

/// The most messages a community's history keeps, with the reactions to them: what the HUD shows
/// of a conversation (`MAX_CHAT_LINES`). A DM history is not trimmed.
const MAX_COMMUNITY_HISTORY: usize = 200;
/// The most records (messages and reactions) a community's history keeps: reactions alone would
/// otherwise grow it without a new message ever moving the message cut.
const MAX_COMMUNITY_RECORDS: usize = 1000;

/// Pending store operations and the one in flight.
#[derive(Resource, Default)]
pub struct DmStore {
    queue: VecDeque<DmStoreOp>,
    running: Option<Task<()>>,
}

impl DmStoreOp {
    async fn run(self) {
        match self {
            DmStoreOp::Append {
                account,
                partner,
                entry,
            } => {
                if let Err(e) = platform::dm_history_append(&account, &partner, &entry).await {
                    warn!("failed to store dm: {e}");
                }
            }
            DmStoreOp::Read {
                account,
                partner,
                sender,
            } => {
                let entries = match platform::dm_history_read(&account, &partner).await {
                    Ok(entries) => entries,
                    Err(e) => {
                        warn!("failed to read dm history with {partner}: {e}");
                        Vec::new()
                    }
                };
                sender.send(fold_reactions(entries));
            }
            DmStoreOp::Delete { account, partner } => {
                if let Err(e) = platform::dm_history_delete(&account, &partner).await {
                    warn!("failed to delete dm history with {partner}: {e}");
                }
            }
            DmStoreOp::Trim {
                account,
                partner,
                keep,
                keep_records,
            } => {
                if let Err(e) =
                    platform::dm_history_trim(&account, &partner, keep, keep_records).await
                {
                    warn!("failed to trim history of {partner}: {e}");
                }
            }
        }
    }
}

/// The stored DMs, oldest first, each with the reactions recorded after it.
fn fold_reactions(entries: Vec<DmHistoryEntry>) -> Vec<DmHistoryEntryData> {
    let mut dms = Vec::<DmHistoryEntryData>::new();
    let mut by_id = HashMap::<String, usize>::new();
    for entry in entries {
        let Some(reaction) = entry.reaction else {
            let message_id = entry
                .sent_at
                .zip(entry.from.as_str().as_h160())
                .map(|(timestamp, from)| chat_message_id(from, timestamp))
                .unwrap_or_default();
            if !message_id.is_empty() {
                by_id.insert(message_id.clone(), dms.len());
            }
            dms.push(DmHistoryEntryData {
                from: entry.from,
                message: entry.message,
                received_at: entry.received_at,
                message_id,
                reactions: Vec::new(),
            });
            continue;
        };
        let Some(&dm) = by_id.get(&reaction.message_id) else {
            continue;
        };
        let reactions = &mut dms[dm].reactions;
        let existing = reactions.iter().position(|r| r.emoji == reaction.emoji);
        match (existing, reaction.remove) {
            (Some(i), true) => {
                reactions[i].from.retain(|from| *from != entry.from);
                if reactions[i].from.is_empty() {
                    reactions.remove(i);
                }
            }
            (Some(i), false) => {
                if !reactions[i].from.contains(&entry.from) {
                    reactions[i].from.push(entry.from);
                }
            }
            (None, false) => reactions.push(DmReactionData {
                emoji: reaction.emoji,
                from: vec![entry.from],
            }),
            (None, true) => (),
        }
    }
    dms
}

/// Starts the next queued store operation once the previous one has finished.
pub fn run_dm_store(mut store: ResMut<DmStore>) {
    if let Some(mut task) = store.running.take() {
        if task.complete().is_none() {
            store.running = Some(task);
            return;
        }
    }
    if let Some(op) = store.queue.pop_front() {
        store.running = Some(IoTaskPool::get().spawn(op.run()));
    }
}

fn now_unix() -> f64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// The store key of a conversation: the partner's wallet in lowercase hex, or a community's
/// channel; `None` for a channel that is neither (Nearby, System).
pub fn history_key(channel: &str) -> Option<String> {
    match PrivateChatTarget::parse(channel)? {
        PrivateChatTarget::Dm(address) => Some(format!("{address:#x}")),
        PrivateChatTarget::Community(id) => Some(channel_for(&id)),
    }
}

/// Stores every DM or community message sent or received, and every reaction to one, under the
/// local account, except from blocked senders.
#[allow(clippy::too_many_arguments)]
pub fn record_dms(
    mut chats: EventReader<ChatEvent>,
    mut received: EventReader<PrivateChatReceived>,
    mut community_received: EventReader<CommunityChatReceived>,
    mut reactions: EventReader<ChatReactionEvent>,
    player: Query<Entity, With<PrimaryUser>>,
    wallet: Res<Wallet>,
    social: Res<SocialClient>,
    mut store: ResMut<DmStore>,
) {
    let Some(account) = wallet.address().map(|a| format!("{a:#x}")) else {
        chats.clear();
        received.clear();
        community_received.clear();
        reactions.clear();
        return;
    };
    let player = player.single().ok();

    let mut entries: Vec<(String, DmHistoryEntry)> = Vec::new();
    for ev in chats.read() {
        if Some(ev.sender) != player {
            continue;
        }
        let Some(partner) = history_key(&ev.channel) else {
            continue;
        };
        entries.push((
            partner,
            DmHistoryEntry {
                from: account.clone(),
                message: ev.message.clone(),
                received_at: now_unix(),
                sent_at: Some(ev.timestamp),
                reaction: None,
            },
        ));
    }
    for dm in received.read() {
        if blocked_by_me(&social, dm.from) {
            continue;
        }
        let partner = format!("{:#x}", dm.from);
        entries.push((
            partner.clone(),
            DmHistoryEntry {
                from: partner,
                message: dm.message.clone(),
                received_at: now_unix(),
                sent_at: Some(dm.timestamp),
                reaction: None,
            },
        ));
    }
    for chat in community_received.read() {
        if blocked_by_me(&social, chat.from) {
            continue;
        }
        entries.push((
            channel_for(&chat.community_id),
            DmHistoryEntry {
                from: format!("{:#x}", chat.from),
                message: chat.message.clone(),
                received_at: now_unix(),
                sent_at: Some(chat.timestamp),
                reaction: None,
            },
        ));
    }
    for ev in reactions.read() {
        if blocked_by_me(&social, ev.from) {
            continue;
        }
        let Some(partner) = history_key(&ev.channel) else {
            continue;
        };
        entries.push((
            partner,
            DmHistoryEntry {
                from: format!("{:#x}", ev.from),
                message: String::new(),
                received_at: now_unix(),
                sent_at: None,
                reaction: Some(DmHistoryReaction {
                    message_id: ev.message_id.clone(),
                    emoji: ev.emoji.clone(),
                    remove: ev.remove,
                }),
            },
        ));
    }

    // a community's history is trimmed behind the frame's appends, once per community
    let mut communities = Vec::<String>::new();
    for (partner, entry) in entries {
        if partner.starts_with(comms::community_chat::CHANNEL_PREFIX)
            && !communities.contains(&partner)
        {
            communities.push(partner.clone());
        }
        store.queue.push_back(DmStoreOp::Append {
            account: account.clone(),
            partner,
            entry,
        });
    }
    for partner in communities {
        store.queue.push_back(DmStoreOp::Trim {
            account: account.clone(),
            partner,
            keep: MAX_COMMUNITY_HISTORY,
            keep_records: MAX_COMMUNITY_RECORDS,
        });
    }
}

pub fn handle_dm_history_requests(
    mut requests: EventReader<SystemApi>,
    wallet: Res<Wallet>,
    mut store: ResMut<DmStore>,
) {
    for request in requests.read() {
        match request {
            SystemApi::GetDmHistory(partner, sender) => {
                let sender = sender.clone();
                let (Some(account), Some(partner)) = (wallet.address(), history_key(partner))
                else {
                    sender.send(Vec::new());
                    continue;
                };
                store.queue.push_back(DmStoreOp::Read {
                    account: format!("{account:#x}"),
                    partner,
                    sender,
                });
            }
            SystemApi::DeleteDmHistory(partner) => {
                let (Some(account), Some(partner)) = (wallet.address(), history_key(partner))
                else {
                    continue;
                };
                store.queue.push_back(DmStoreOp::Delete {
                    account: format!("{account:#x}"),
                    partner,
                });
            }
            _ => (),
        }
    }
}

/// `/dm_history <address|community:<id>> [count]` prints the last `count` (default 20) stored
/// messages of a conversation.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/dm_history")]
pub struct DmHistoryCommand {
    address: String,
    count: Option<usize>,
}

pub fn dm_history(
    mut input: ConsoleCommand<DmHistoryCommand>,
    mut events: EventWriter<SystemApi>,
    mut pending: ResMut<PendingConsoleResponses>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let count = command.count.unwrap_or(20);
    let (sx, rx) = RpcResultSender::<Vec<DmHistoryEntryData>>::channel();
    events.write(SystemApi::GetDmHistory(command.address, sx));
    let responder = input.take_responder();
    pending.push_receiver(
        rx,
        move |entries| {
            let skipped = entries.len().saturating_sub(count);
            Ok(entries
                .iter()
                .skip(skipped)
                .map(|e| format!("[{}] {}: {}", e.received_at, e.from, e.message))
                .collect::<Vec<_>>()
                .join("\n"))
        },
        responder,
    );
}

/// `/dm_delete <address|community:<id>>` deletes a conversation's stored messages.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/dm_delete")]
pub struct DmDeleteCommand {
    address: String,
}

pub fn dm_delete(mut input: ConsoleCommand<DmDeleteCommand>, mut events: EventWriter<SystemApi>) {
    if let Some(Ok(command)) = input.take() {
        events.write(SystemApi::DeleteDmHistory(command.address));
        input.reply_ok("");
    }
}

#[cfg(test)]
mod tests {
    use super::history_key;

    #[test]
    fn conversations_are_keyed_by_wallet_or_community_channel() {
        assert_eq!(
            history_key("0xABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCD").as_deref(),
            Some("0xabcdefabcdefabcdefabcdefabcdefabcdefabcd")
        );
        assert_eq!(
            history_key("community:ABC-1").as_deref(),
            Some("community:abc-1")
        );
        assert_eq!(history_key("Nearby"), None);
        assert_eq!(history_key("System"), None);
        assert_eq!(history_key("community:"), None);
    }
}
