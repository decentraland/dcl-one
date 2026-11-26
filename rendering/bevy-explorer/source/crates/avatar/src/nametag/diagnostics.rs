//! The nametags' console commands and their section of the debug panel.

use alloy_core::primitives::Address;
use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::{
    structs::{AppConfig, PrimaryCamera},
    util::AsH160,
};
use comms::global_crdt::ForeignPlayer;
use console::{DebugInput, DebugInputKind, DebugSection};
use scene_runner::update_world::text_shape::Page;
use serde_json::{json, Value};

use super::{
    bubble::{BubbleKind, ChatBubbleRequest, ChatBubbleRetext},
    layout::Voice,
    plate::NametagPlate,
    tag_opacity, tag_scale,
    text::TextShaper,
    ChatBubble, Nametag, NametagContent, NametagVoice, OfficialWallets, SceneAvatarNametag,
};
use crate::dynamic_nametag::{NametagHeadOffset, NametagsVisible};

pub const SECTION: DebugSection = DebugSection {
    title: "Nametags",
    query: "/nametag_state",
    fields: &[
        ("Shown", "shown"),
        ("Of scene avatars", "sceneAvatars"),
        ("With a scene's plate", "plates"),
        ("Hidden", "hidden"),
        ("Glyph atlas (distance field / colour px)", "atlas"),
        ("Chat bubbles and reactions", "bubbles"),
    ],
    actions: &[(
        "Show a test bubble",
        "/nametag_bubble all nearby A test message from the debug panel",
    )],
    inputs: &[DebugInput {
        label: "Show nametags",
        field: "visible",
        command: "/nametags {value}",
        kind: DebugInputKind::Toggle,
    }],
};

/// Report the nametags as JSON: `visible` (the nametags toggle), `shown`, `sceneAvatars` (of
/// those shown, the ones over a scene's avatars), `plates` (of those shown, the ones with a
/// scene's `AvatarNametag` plate) and `hidden` (the tags that don't show: out
/// of range, blocked, or hidden by the scene or a setting), `atlas` (the glyph atlas'
/// distance-field and colour page sizes in pixels, 0 for a page not made), `bubbles` (the chat
/// bubble setting) and `tags`, one per shown tag: `player` (an address, or `local`, or null
/// for an avatar with no wallet), `sceneAvatar` (the tag is over an avatar a scene made),
/// `nameShown` (false for a tag that is its plate alone: the avatar has no name, or a scene's
/// area hides it), `plate` (null, or the plate's `label` and its `labelColor`, `backgroundColor`
/// and `borderColor` as `#RRGGBB`), `name` and `suffix` (empty for an avatar with no name),
/// `verified`, `official`, `voice` (`off`, `idle`, `speaking`, `hushed`),
/// `distance` in metres from the camera, `opacity` (0 to 1), `scale` (metres per tag unit),
/// `position` (world, metres; all four as of the last frame drawn), `headOffset` (how far over
/// the head bone the tag sits, in metres for an avatar at a scale of one; null until the
/// avatar is built) and `bubble` (null, or its `kind`, `mention` and message `length`; never
/// the message).
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/nametag_state")]
pub struct NametagStateCommand;

pub fn nametag_state(
    mut cmd: ConsoleCommand<NametagStateCommand>,
    visible: Res<NametagsVisible>,
    config: Res<AppConfig>,
    mut shaper: TextShaper,
    camera: Query<(&GlobalTransform, &Projection), With<PrimaryCamera>>,
    tags: Query<(&Nametag, &ChildOf, &GlobalTransform, &Visibility)>,
    owners: Query<(
        Option<&NametagContent>,
        Option<&NametagPlate>,
        &NametagVoice,
        Option<&ForeignPlayer>,
        Option<&NametagHeadOffset>,
        Option<&ChatBubble>,
        Has<SceneAvatarNametag>,
    )>,
    local: Res<common::structs::PrimaryPlayerRes>,
) {
    let Some(Ok(_)) = cmd.take() else {
        return;
    };
    let atlas = shaper.atlas();
    let page_size = |page| atlas.page_image(page).map_or(0, |(_, size)| size);
    let atlas = [page_size(Page::Sdf), page_size(Page::Color)];

    // the shader's view of each tag, worked out the same way
    let view = camera.single().ok().map(|(transform, projection)| {
        let fov = match projection {
            Projection::Perspective(perspective) => Some(perspective.fov),
            _ => None,
        };
        (transform.translation(), fov)
    });

    let mut hidden = 0;
    let mut shown = Vec::new();
    for (nametag, owner, transform, visibility) in &tags {
        if !nametag.shown || *visibility == Visibility::Hidden {
            hidden += 1;
            continue;
        }
        let Ok((content, plate, voice, foreign, head_offset, bubble, scene_avatar)) =
            owners.get(owner.parent())
        else {
            continue;
        };
        let hex = |color: Color| color.to_srgba().to_hex();
        let player = match foreign {
            Some(player) => json!(format!("{:#x}", player.address)),
            None if owner.parent() == local.0 => json!("local"),
            None => Value::Null,
        };
        let position = transform.translation();
        let distance = view.map(|(camera, _)| camera.distance(position));
        let scale = distance
            .zip(view.and_then(|(_, fov)| fov))
            .map(|(distance, fov)| tag_scale(distance, fov));
        shown.push(json!({
            "player": player,
            "sceneAvatar": scene_avatar,
            "nameShown": nametag.name_shown,
            "plate": plate.map(|plate| json!({
                "label": plate.label,
                "labelColor": hex(plate.label_color),
                "backgroundColor": hex(plate.background),
                "borderColor": hex(plate.border),
            })),
            "name": content.map_or("", |content| &content.name),
            "suffix": content.map_or("", |content| &content.suffix),
            "verified": content.is_some_and(|content| content.verified),
            "official": content.is_some_and(|content| content.official),
            "voice": match voice.0 {
                Voice::Off => "off",
                Voice::Idle => "idle",
                Voice::Speaking => "speaking",
                Voice::Hushed => "hushed",
            },
            "distance": distance,
            "opacity": distance.map(tag_opacity),
            "scale": scale,
            "position": position.to_array(),
            "headOffset": head_offset.map(|offset| offset.0),
            "bubble": bubble.map(|bubble| json!({
                "kind": match bubble.kind {
                    BubbleKind::Nearby => "nearby",
                    BubbleKind::Community(_) => "community",
                    BubbleKind::Dm { .. } => "dm",
                },
                "mention": bubble.mention,
                "length": bubble.message.text.chars().count(),
            })),
        }));
    }

    cmd.reply_ok(
        json!({
            "visible": visible.0,
            "shown": shown.len(),
            "sceneAvatars": shown.iter().filter(|tag| tag["sceneAvatar"] == true).count(),
            "plates": shown.iter().filter(|tag| !tag["plate"].is_null()).count(),
            "hidden": hidden,
            "atlas": atlas,
            "bubbles": format!("{:?}", config.chat_bubble_visibility),
            "tags": shown,
        })
        .to_string(),
    );
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum TestBubbleKind {
    Nearby,
    Community,
    Dm,
}

/// Show `message` in a chat bubble over a nametag, as a message of `kind` (`nearby`,
/// `community`, `dm`) would: `target` is `local`, a player's address, or `all` for every tag
/// showing. Nothing is sent to anyone. Replies with the number of bubbles asked for as
/// `bubbles`; the chat bubble setting still decides whether they show.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/nametag_bubble")]
pub struct NametagBubbleCommand {
    target: String,
    kind: TestBubbleKind,
    /// The rest of the line.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    message: Vec<String>,
}

pub fn nametag_bubble(
    mut cmd: ConsoleCommand<NametagBubbleCommand>,
    local: Res<common::structs::PrimaryPlayerRes>,
    tags: Query<(&Nametag, &ChildOf)>,
    players: Query<Option<&ForeignPlayer>>,
    mut requests: EventWriter<ChatBubbleRequest>,
) {
    let Some(Ok(command)) = cmd.take() else {
        return;
    };
    let address = command.target.as_h160();
    if address.is_none() && !matches!(command.target.as_str(), "local" | "all") {
        cmd.reply_failed("target must be `local`, `all` or an address");
        return;
    }
    let kind = match command.kind {
        TestBubbleKind::Nearby => BubbleKind::Nearby,
        TestBubbleKind::Community => BubbleKind::Community(Some("Test community".to_owned())),
        TestBubbleKind::Dm => BubbleKind::Dm { recipient: None },
    };
    let senders = tags
        .iter()
        .filter(|(nametag, _)| nametag.shown && nametag.name_shown)
        .map(|(_, owner)| owner.parent())
        .filter(|player| match address {
            Some(address) => players
                .get(*player)
                .ok()
                .flatten()
                .is_some_and(|foreign| foreign.address == address),
            None => command.target == "all" || *player == local.0,
        })
        .collect::<Vec<_>>();
    for (index, sender) in senders.iter().enumerate() {
        requests.write(ChatBubbleRequest {
            sender: *sender,
            kind: kind.clone(),
            message: command.message.join(" "),
            message_id: format!("test-{index}"),
        });
    }
    cmd.reply_ok(json!({ "bubbles": senders.len() }).to_string());
}

/// Replace the text of the chat bubble showing the message `message_id` (a translation, or
/// the original back). Replies with nothing; a message no bubble shows is ignored.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/chat_bubble_text")]
pub struct ChatBubbleTextCommand {
    message_id: String,
    /// The rest of the line.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    message: Vec<String>,
}

pub fn chat_bubble_text(
    mut cmd: ConsoleCommand<ChatBubbleTextCommand>,
    mut retexts: EventWriter<ChatBubbleRetext>,
) {
    let Some(Ok(command)) = cmd.take() else {
        return;
    };
    retexts.write(ChatBubbleRetext {
        message_id: command.message_id,
        message: command.message.join(" "),
    });
    cmd.reply_ok("");
}

/// Read or set whether a wallet's nametag carries the official badge, for this session.
/// Replies with `address` and `official`.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/nametag_official")]
pub struct NametagOfficialCommand {
    address: String,
    official: Option<bool>,
}

pub fn nametag_official(
    mut cmd: ConsoleCommand<NametagOfficialCommand>,
    mut wallets: ResMut<OfficialWallets>,
) {
    let Some(Ok(command)) = cmd.take() else {
        return;
    };
    let Some(address): Option<Address> = command.address.as_h160() else {
        cmd.reply_failed("not an address");
        return;
    };
    match command.official {
        Some(true) => {
            wallets.0.insert(address);
        }
        Some(false) => {
            wallets.0.remove(&address);
        }
        None => {}
    }
    cmd.reply_ok(
        json!({
            "address": format!("{address:#x}"),
            "official": wallets.0.contains(&address),
        })
        .to_string(),
    );
}
