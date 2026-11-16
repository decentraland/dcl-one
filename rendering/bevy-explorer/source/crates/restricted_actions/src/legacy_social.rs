use std::collections::VecDeque;

use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
#[cfg(not(test))]
use bevy_console::ConsoleCommand;
use common::{
    profile::AvatarColor,
    rpc::{RpcCall, RpcCallEvent, RpcEventSender, RpcOrigin},
    sets::SceneSets,
    structs::{EmoteLifecycle, EmoteLifecycleEvent, EmoteLifecycleSource},
};
use comms::{
    global_crdt::{CrdtContexts, ForeignPlayer, PlayerPositionEvent},
    profile::UserProfile,
    public_voice::PublicVoiceParticipants,
};
#[cfg(not(test))]
use console::DoAddConsoleCommand;
use console::{AddDebugSection, DebugSection};
use scene_runner::renderer_context::RendererSceneContext;
use serde_json::{json, Value};

pub struct LegacySocialPlugin;

impl Plugin for LegacySocialPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                subscribe,
                attach_peer_state,
                profiles_and_visibility,
                positions,
                expressions,
                talking,
                pull,
            )
                .chain()
                .in_set(SceneSets::RestrictedActions),
        )
        .add_observer(removed);
        app.add_debug_section(DebugSection {
            title: "Legacy avatar events",
            query: "/legacy_social",
            fields: &[
                ("Subscribed scenes", "subscribedScenes"),
                ("Queued events", "queuedEvents"),
                ("Scenes awaiting resync", "resyncScenes"),
                ("Tracked peers", "trackedPeers"),
            ],
            actions: &[],
            inputs: &[],
        });
        #[cfg(not(test))]
        app.add_console_command::<LegacySocialCommand, _>(legacy_social_command);
    }
}

#[cfg(not(test))]
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/legacy_social")]
struct LegacySocialCommand;

#[cfg(not(test))]
fn legacy_social_command(
    mut input: ConsoleCommand<LegacySocialCommand>,
    subscribers: Query<&LegacySocialSubscription>,
    peers: Query<(), With<LegacyPeerState>>,
) {
    if let Some(Ok(_)) = input.take() {
        input.reply_ok(
            json!({
                "subscribedScenes": subscribers.iter().count(),
                "queuedEvents": subscribers.iter().map(|s| s.pending.len()).sum::<usize>(),
                "resyncScenes": subscribers.iter().filter(|s| s.resync).count(),
                "trackedPeers": peers.iter().count(),
            })
            .to_string(),
        );
    }
}

#[derive(Component)]
struct LegacySocialSubscription {
    hash: String,
    context: Option<Entity>,
    sender: RpcEventSender,
    talking: HashMap<String, bool>,
    pending: VecDeque<LegacyEvent>,
    delivered: HashSet<String>,
    resync: bool,
    fresh: bool,
}

#[derive(Component, Default)]
struct LegacyPeerState {
    profile: Option<Value>,
    position: Option<Value>,
    position_index: u32,
    visible: bool,
}

const MAX_PENDING: usize = 1024;

struct LegacyEvent {
    event: &'static str,
    user_id: String,
    payload: Value,
}

impl LegacyEvent {
    fn encode(&self) -> String {
        json!({"event": self.event, "payload": self.payload.to_string()}).to_string()
    }
}

fn emit(
    subscription: &mut LegacySocialSubscription,
    event: &'static str,
    user_id: &str,
    mut payload: Value,
) {
    if subscription.resync {
        return;
    }
    if subscription.pending.len() == MAX_PENDING {
        subscription.pending.clear();
        subscription.resync = true;
        return;
    }
    payload["type"] = event.into();
    payload["userId"] = user_id.into();
    subscription.pending.push_back(LegacyEvent {
        event,
        user_id: user_id.to_owned(),
        payload,
    });
}

fn subscribe(
    mut commands: Commands,
    mut requests: EventReader<RpcCallEvent>,
    scenes: Query<&RendererSceneContext>,
    mut subscribers: Query<(Entity, &mut LegacySocialSubscription)>,
    contexts: Res<CrdtContexts>,
) {
    for (entity, mut subscription) in &mut subscribers {
        if subscription.sender.is_closed()
            || !scenes
                .get(entity)
                .is_ok_and(|scene| scene.hash.as_ref() == subscription.hash)
        {
            commands.entity(entity).remove::<LegacySocialSubscription>();
        }
        let context = contexts.try_for_scene_hash(&subscription.hash);
        if subscription.context != context {
            subscription.context = context;
            subscription.pending.clear();
            subscription.talking.clear();
            subscription.resync = true;
        }
    }
    for request in requests.read() {
        let RpcOrigin::Scene { entity, hash } = &request.origin else {
            continue;
        };
        let sender = match &request.call {
            RpcCall::SubscribeLegacyAvatarEvents { sender } => sender,
            _ => continue,
        };
        if sender.is_closed() || !scenes.get(*entity).is_ok_and(|scene| scene.hash == *hash) {
            continue;
        }
        if subscribers.get(*entity).is_ok_and(|(_, subscription)| {
            subscription.hash == hash.as_ref() && !subscription.sender.is_closed()
        }) {
            continue;
        }
        commands.entity(*entity).insert(LegacySocialSubscription {
            hash: hash.to_string(),
            context: contexts.try_for_scene_hash(hash),
            sender: sender.clone(),
            talking: HashMap::default(),
            pending: VecDeque::new(),
            delivered: HashSet::default(),
            resync: false,
            fresh: true,
        });
    }
}

fn attach_peer_state(
    mut commands: Commands,
    peers: Query<Entity, (With<ForeignPlayer>, Without<LegacyPeerState>)>,
) {
    for peer in &peers {
        commands.entity(peer).insert(LegacyPeerState::default());
    }
}

fn user_data(subscription: &mut LegacySocialSubscription, user_id: &str, state: &LegacyPeerState) {
    let Some(profile) = &state.profile else {
        return;
    };
    let mut data = json!({
        "ethereumAddress": user_id,
        "baseUrl": profile["baseUrl"],
        "visible": state.visible,
    });
    if let Some(position) = &state.position {
        data["position"] = position.clone();
    }
    emit(
        subscription,
        "USER_DATA",
        user_id,
        json!({"data": data, "profile": profile}),
    );
}

#[allow(clippy::type_complexity)]
fn profiles_and_visibility(
    mut peers: Query<(
        &ForeignPlayer,
        Option<Ref<UserProfile>>,
        Option<&Visibility>,
        Option<&Children>,
        &mut LegacyPeerState,
    )>,
    mut subscribers: Query<&mut LegacySocialSubscription>,
    rendered: Query<&Visibility, With<avatar::AvatarProcessed>>,
    contexts: Res<CrdtContexts>,
) {
    for (peer, profile, visibility, children, mut state) in &mut peers {
        if peer.address.is_zero() {
            continue;
        }
        let user_id = format!("{:#x}", peer.address);
        let mut rendered_visibility = children
            .into_iter()
            .flat_map(|children| children.iter())
            .filter_map(|child| rendered.get(child).ok())
            .map(|visibility| *visibility != Visibility::Hidden)
            .peekable();
        let visible = visibility != Some(&Visibility::Hidden)
            && (rendered_visibility.peek().is_none() || rendered_visibility.any(|visible| visible));
        let visibility_changed = state.visible != visible;
        let mut profile_changed = false;
        if let Some(profile) =
            profile.filter(|profile| profile.content.eth_address.eq_ignore_ascii_case(&user_id))
        {
            if profile.is_changed() || state.profile.is_none() {
                let profile = renderer_profile(&profile, &user_id);
                profile_changed = state.profile.as_ref() != Some(&profile);
                state.profile = Some(profile);
            }
        } else {
            state.profile = None;
        }
        state.visible = visible;
        for mut subscription in &mut subscribers {
            if contexts.try_for_scene_hash(&subscription.hash) != Some(peer.context) {
                continue;
            }
            if subscription.fresh || visibility_changed {
                emit(
                    &mut subscription,
                    "USER_VISIBLE",
                    &user_id,
                    json!({"visible": visible}),
                );
            }
            if subscription.fresh || profile_changed || (visibility_changed && visible) {
                user_data(&mut subscription, &user_id, &state);
            }
        }
    }
    for mut subscription in &mut subscribers {
        subscription.fresh = false;
    }
}

fn positions(
    mut events: EventReader<PlayerPositionEvent>,
    mut peers: Query<(&ForeignPlayer, &mut LegacyPeerState)>,
    mut subscribers: Query<&mut LegacySocialSubscription>,
    contexts: Res<CrdtContexts>,
) {
    for event in events.read() {
        let Ok((peer, mut state)) = peers.get_mut(event.player) else {
            continue;
        };
        if peer.address.is_zero() {
            continue;
        }
        state.position_index = state.position_index.wrapping_add(1);
        let [x, y, z] = event.translation.0;
        let [qx, qy, qz, qw] = event.rotation.0;
        state.position = Some(json!({
            "index": state.position_index,
            "positionX": x, "positionY": y, "positionZ": z,
            "rotationX": qx, "rotationY": qy, "rotationZ": qz, "rotationW": qw,
        }));
        let user_id = format!("{:#x}", peer.address);
        for mut subscription in &mut subscribers {
            if contexts.try_for_scene_hash(&subscription.hash) == Some(peer.context) {
                user_data(&mut subscription, &user_id, &state);
            }
        }
    }
}

fn expressions(
    mut events: EventReader<EmoteLifecycleEvent>,
    peers: Query<&ForeignPlayer>,
    mut subscribers: Query<&mut LegacySocialSubscription>,
    contexts: Res<CrdtContexts>,
) {
    for event in events.read() {
        if event.source != EmoteLifecycleSource::Wire {
            continue;
        }
        let (Ok(peer), EmoteLifecycle::Started { urn, .. }) =
            (peers.get(event.avatar), &event.event)
        else {
            continue;
        };
        if peer.address.is_zero() {
            continue;
        }
        let timestamp = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        for mut subscription in &mut subscribers {
            if contexts.try_for_scene_hash(&subscription.hash) == Some(peer.context) {
                emit(
                    &mut subscription,
                    "USER_EXPRESSION",
                    &format!("{:#x}", peer.address),
                    json!({"expressionId": urn, "timestamp": timestamp}),
                );
            }
        }
    }
}

fn talking(
    mut subscribers: Query<&mut LegacySocialSubscription>,
    peers: Query<&ForeignPlayer>,
    contexts: Res<CrdtContexts>,
    voices: PublicVoiceParticipants,
) {
    if subscribers.is_empty() {
        return;
    }
    let snapshot = voices.snapshot();
    for mut subscription in &mut subscribers {
        let Some(context) = contexts.try_for_scene_hash(&subscription.hash) else {
            subscription.talking.clear();
            continue;
        };
        let mut current = HashSet::<String>::default();
        for peer in &peers {
            if peer.context != context || peer.address.is_zero() {
                continue;
            }
            let user_id = format!("{:#x}", peer.address);
            let active = snapshot.is_speaking(
                &user_id,
                &subscription.hash,
                context,
                context == contexts.shared(),
            );
            let previous = subscription.talking.insert(user_id.clone(), active);
            if previous.is_some_and(|previous| previous != active) || (previous.is_none() && active)
            {
                emit(
                    &mut subscription,
                    "USER_TALKING",
                    &user_id,
                    json!({"talking": active}),
                );
            }
            current.insert(user_id);
        }
        subscription
            .talking
            .retain(|user_id, _| current.contains(user_id));
    }
}

fn removed(
    trigger: Trigger<OnRemove, ForeignPlayer>,
    mut commands: Commands,
    peers: Query<&ForeignPlayer>,
    mut subscribers: Query<&mut LegacySocialSubscription>,
    contexts: Res<CrdtContexts>,
) {
    let Ok(peer) = peers.get(trigger.target()) else {
        return;
    };
    if peer.address.is_zero() {
        return;
    }
    for mut subscription in &mut subscribers {
        if contexts.try_for_scene_hash(&subscription.hash) == Some(peer.context) {
            subscription.talking.remove(&format!("{:#x}", peer.address));
            emit(
                &mut subscription,
                "USER_REMOVED",
                &format!("{:#x}", peer.address),
                json!({}),
            );
        }
    }
    commands
        .entity(trigger.target())
        .try_remove::<LegacyPeerState>();
}

fn pull(
    mut requests: EventReader<RpcCallEvent>,
    mut subscribers: Query<&mut LegacySocialSubscription>,
    peers: Query<(&ForeignPlayer, &LegacyPeerState)>,
    contexts: Res<CrdtContexts>,
    voices: PublicVoiceParticipants,
) {
    let snapshot = voices.snapshot();
    for request in requests.read() {
        let RpcCall::PullLegacyAvatarEvents { response, .. } = &request.call else {
            continue;
        };
        if response.is_cancelled() {
            continue;
        }
        let RpcOrigin::Scene { entity, hash } = &request.origin else {
            response.send(Vec::new());
            continue;
        };
        let Ok(mut subscription) = subscribers.get_mut(*entity) else {
            response.send(Vec::new());
            continue;
        };
        if subscription.hash != hash.as_ref() || subscription.sender.is_closed() {
            response.send(Vec::new());
            continue;
        }
        if subscription.resync {
            subscription.resync = false;
            let delivered = std::mem::take(&mut subscription.delivered);
            for user_id in delivered {
                emit(&mut subscription, "USER_REMOVED", &user_id, json!({}));
            }
            let context = contexts.try_for_scene_hash(hash);
            // A context has at most 224 ForeignPlayer ids. Removals plus three current
            // state messages per peer fit inside the 1024-event response bound.
            for (peer, state) in &peers {
                if Some(peer.context) != context || peer.address.is_zero() {
                    continue;
                }
                let user_id = format!("{:#x}", peer.address);
                emit(
                    &mut subscription,
                    "USER_VISIBLE",
                    &user_id,
                    json!({"visible": state.visible}),
                );
                user_data(&mut subscription, &user_id, state);
                let talking = snapshot.is_speaking(
                    &user_id,
                    hash,
                    peer.context,
                    peer.context == contexts.shared(),
                );
                {
                    emit(
                        &mut subscription,
                        "USER_TALKING",
                        &user_id,
                        json!({"talking": talking}),
                    );
                }
            }
        }
        let pending = std::mem::take(&mut subscription.pending);
        let mut result = Vec::with_capacity(pending.len());
        for event in pending {
            if event.event == "USER_REMOVED" {
                subscription.delivered.remove(&event.user_id);
            } else {
                subscription.delivered.insert(event.user_id.clone());
            }
            result.push(event.encode());
        }
        response.send(result);
    }
}

fn renderer_profile(profile: &UserProfile, user_id: &str) -> Value {
    let content = &profile.content;
    let avatar = &content.avatar;
    let suffix = format!("#{}", &user_id[user_id.len().saturating_sub(4)..]);
    let name: String = content
        .name
        .strip_suffix(&suffix)
        .unwrap_or(&content.name)
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    let name = if content.has_claimed_name {
        name
    } else {
        name + &suffix
    };
    let extra = &content.extra_fields;
    let snapshots = avatar.snapshots.as_ref();
    json!({
        "userId": user_id, "ethAddress": user_id, "name": name,
        "description": extra.get("description").and_then(Value::as_str).unwrap_or_default(),
        "version": if content.version == 0 { -1 } else { content.version },
        "blocked": extra.get("blocked").filter(|value| value.is_array()).cloned().unwrap_or(json!([])),
        "muted": extra.get("muted").filter(|value| value.is_array()).cloned().unwrap_or(json!([])),
        "inventory": [], "created_at": 0, "updated_at": 0, "email": "",
        "hasConnectedWeb3": content.has_connected_web3.unwrap_or_default(),
        "hasClaimedName": content.has_claimed_name,
        "tutorialFlagsMask": 0, "tutorialStep": extra.get("tutorialStep").and_then(Value::as_u64).unwrap_or_default(),
        "baseUrl": profile.base_url,
        "snapshots": {
            "face256": snapshot(snapshots.map(|s| s.face256.as_str()), "QmSqZ2npVD4RLdqe17FzGCFcN29RfvmqmEd2FcQUctxaKk"),
            "body": snapshot(snapshots.map(|s| s.body.as_str()), "QmSav1o6QK37Jj1yhbmhYk9MJc6c2H5DWbWzPVsg9JLYfF"),
        },
        "avatar": {
            "bodyShape": avatar.body_shape.as_deref().unwrap_or_default(),
            "wearables": avatar.wearables,
            "emotes": avatar.emotes.as_deref().unwrap_or_default(),
            "eyeColor": color(avatar.eyes), "hairColor": color(avatar.hair), "skinColor": color(avatar.skin),
        },
    })
}

fn color(color: Option<AvatarColor>) -> Value {
    match color {
        Some(color) => json!({"r": color.color.r, "g": color.color.g, "b": color.color.b, "a": 1}),
        None => json!({"r": 1, "g": 0, "b": 1, "a": 1}),
    }
}

fn snapshot(value: Option<&str>, fallback: &str) -> String {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return fallback.to_owned();
    };
    if url::Url::parse(value).is_ok()
        || value.starts_with("/images")
        || value.starts_with("Qm")
        || value.starts_with("bafy")
    {
        value.to_owned()
    } else {
        format!("data:text/plain;base64,{value}")
    }
}

#[cfg(test)]
#[path = "legacy_social_tests.rs"]
mod tests;
