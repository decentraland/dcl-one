//! `/avatar_state`: what avatars are selected to wear, what was built from that and what they are
//! playing, as JSON for end-to-end tests (docs/guidelines/running.md, "Test commands").

use alloy_core::primitives::Address;
use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use collectibles::{wearables::WearableCategory, Collectibles, Emote};
use common::{
    structs::{EmoteMask, PrimaryUser},
    util::AsH160,
};
use comms::{global_crdt::ForeignPlayer, profile::UserProfile};
use scene_runner::{initialize_scene::SuperUserScene, SceneEntity};
use serde_json::{json, Value};

use crate::{
    animate::{ActiveEmote, ActiveEmoteSource, EmotePlayback, MaskedEmote},
    avatar_texture::BoothAvatar,
    AvatarDefinition, AvatarLoaded, AvatarProcessed, AvatarSelection, PreviousAvatar,
};

/// Report, as JSON, what avatars are selected to wear, what was built from that and the emote they
/// play: the local player's, all of them, or the player with an address
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/avatar_state")]
pub struct AvatarStateCommand {
    /// `all`, or a player's address; the local player's avatar by default
    target: Option<String>,
}

/// What an avatar is. Declared in the order a report lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    /// the local player
    Player,
    /// a player connected through comms
    OtherPlayer,
    /// an avatar a scene put in its own world (an NPC)
    SceneAvatar,
    /// the HUD's own avatar: one of a super-user scene (the bridge scene draws the Backpack's and
    /// the lobby's) or a photo booth's
    Preview,
    /// none of the above
    Other,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Player => "player",
            Kind::OtherPlayer => "otherPlayer",
            Kind::SceneAvatar => "sceneAvatar",
            Kind::Preview => "preview",
            Kind::Other => "other",
        }
    }
}

#[derive(Debug, PartialEq)]
enum Target {
    Player,
    All,
    Address(Address),
}

impl Target {
    fn parse(arg: Option<&str>) -> Result<Self, String> {
        match arg {
            None => Ok(Self::Player),
            Some("all") => Ok(Self::All),
            Some(address) => address
                .as_h160()
                .map(Self::Address)
                .ok_or_else(|| format!("`{address}` is neither `all` nor an address")),
        }
    }

    fn selects(&self, kind: Kind, address: Option<Address>) -> bool {
        match self {
            Target::Player => kind == Kind::Player,
            Target::All => true,
            Target::Address(wanted) => address == Some(*wanted),
        }
    }
}

/// One avatar's report, as plain data
struct AvatarReport {
    kind: Kind,
    address: Option<Address>,
    selected: Selected,
    /// `None` until the avatar's look has resolved far enough to start building
    rendered: Option<Rendered>,
    /// the full-body slot; `None` until the avatar has animated once
    emote: Option<EmoteState>,
    /// the upper-body emote overlaying it, while one plays
    upper_body_emote: Option<EmoteState>,
}

/// What the avatar is told to wear, as written: not resolved, not defaulted
struct Selected {
    body_shape: Option<String>,
    wearables: Vec<String>,
    emotes: Vec<String>,
    force_render: Vec<String>,
    /// a scene's avatar shape overrides the profile's
    from_scene: bool,
}

/// What was built: the wearables that show (the selection's resolved and defaulted, less what
/// others hide) and what is hidden
struct Rendered {
    processed: bool,
    /// processed without ever loading: nothing renders
    failed: bool,
    body_shape: String,
    /// (category slot, urn)
    wearables: Vec<(&'static str, String)>,
    hidden: Vec<&'static str>,
}

struct EmoteState {
    urn: String,
    source: &'static str,
    repeat: bool,
    finished: bool,
    mask: &'static str,
}

impl AvatarReport {
    fn to_json(&self) -> Value {
        json!({
            "kind": self.kind.name(),
            "address": self.address.map(|address| format!("{address:#x}")),
            "selected": {
                "bodyShape": self.selected.body_shape,
                "wearables": self.selected.wearables,
                "emotes": self.selected.emotes,
                "forceRender": self.selected.force_render,
                "fromScene": self.selected.from_scene,
            },
            "rendered": self.rendered.as_ref().map(|rendered| json!({
                "processed": rendered.processed,
                "failed": rendered.failed,
                "bodyShape": rendered.body_shape,
                "wearables": rendered
                    .wearables
                    .iter()
                    .map(|(category, urn)| json!({ "category": category, "urn": urn }))
                    .collect::<Vec<_>>(),
                "hidden": rendered.hidden,
            })),
            "emote": self.emote.as_ref().map(EmoteState::to_json),
            "upperBodyEmote": self.upper_body_emote.as_ref().map(EmoteState::to_json),
        })
    }
}

impl EmoteState {
    fn to_json(&self) -> Value {
        json!({
            "urn": self.urn,
            "source": self.source,
            "repeat": self.repeat,
            "finished": self.finished,
            "mask": self.mask,
        })
    }
}

fn source_name(source: ActiveEmoteSource) -> &'static str {
    match source {
        ActiveEmoteSource::VelocitySelected => "velocitySelected",
        ActiveEmoteSource::TriggeredEmote => "triggeredEmote",
        ActiveEmoteSource::SceneMovementAnim => "sceneMovementAnim",
    }
}

fn mask_name(mask: EmoteMask) -> &'static str {
    match mask {
        EmoteMask::FullBody => "fullBody",
        EmoteMask::UpperBody => "upperBody",
    }
}

/// The slots of the hidden categories, in a fixed order
fn hidden_slots<'a>(hides: impl IntoIterator<Item = &'a WearableCategory>) -> Vec<&'static str> {
    let mut slots: Vec<_> = hides.into_iter().map(|category| category.slot).collect();
    slots.sort_unstable();
    slots
}

#[allow(clippy::type_complexity)]
pub fn avatar_state_cmd(
    mut input: ConsoleCommand<AvatarStateCommand>,
    avatars: Query<(
        Entity,
        &AvatarSelection,
        Option<&Children>,
        Option<&PreviousAvatar>,
        Option<(&ActiveEmote, &MaskedEmote)>,
        Has<PrimaryUser>,
        Option<&ForeignPlayer>,
        Option<&SceneEntity>,
        Has<BoothAvatar>,
        Option<&UserProfile>,
    )>,
    renders: Query<(&AvatarDefinition, Has<AvatarProcessed>, Has<AvatarLoaded>)>,
    super_user_scenes: Query<(), With<SuperUserScene>>,
    // read-only, so the command's every-frame check runs beside the emote systems
    emotes: Res<Collectibles<Emote>>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let target = match Target::parse(command.target.as_deref()) {
        Ok(target) => target,
        Err(message) => {
            input.reply_failed(message);
            return;
        }
    };

    let emote_state = |playback: &EmotePlayback, source: ActiveEmoteSource| EmoteState {
        urn: emotes.source_urn(&playback.urn).to_owned(),
        source: source_name(source),
        repeat: playback.repeat,
        finished: playback.finished,
        mask: mask_name(playback.mask),
    };

    let mut reports = Vec::new();
    for (
        entity,
        selection,
        children,
        previous,
        emote,
        is_primary,
        foreign,
        scene_entity,
        is_booth,
        profile,
    ) in &avatars
    {
        let (kind, address) = if is_primary {
            let address = profile.and_then(|profile| profile.content.eth_address.as_h160());
            (Kind::Player, address)
        } else if let Some(foreign) = foreign {
            (Kind::OtherPlayer, Some(foreign.address))
        } else if let Some(scene_entity) = scene_entity {
            let kind = if super_user_scenes.contains(scene_entity.root) {
                Kind::Preview
            } else {
                Kind::SceneAvatar
            };
            (kind, None)
        } else if is_booth {
            (Kind::Preview, None)
        } else {
            (Kind::Other, None)
        };
        if !target.selects(kind, address) {
            continue;
        }

        // the newest build: while a new look loads, the one it replaces stays on show
        let render = children
            .and_then(|children| {
                children.iter().rfind(|child| {
                    previous.is_none_or(|previous| previous.0 != *child) && renders.contains(*child)
                })
            })
            .and_then(|child| renders.get(child).ok());

        let shape = &selection.shape.0;
        reports.push((
            entity,
            AvatarReport {
                kind,
                address,
                selected: Selected {
                    body_shape: shape.body_shape.clone(),
                    wearables: shape.wearables.clone(),
                    emotes: shape.emotes.clone(),
                    force_render: shape.force_render.clone(),
                    from_scene: selection.scene.is_some(),
                },
                rendered: render.map(|(definition, processed, loaded)| {
                    let definition = &definition.render;
                    Rendered {
                        processed,
                        failed: processed && !loaded,
                        body_shape: definition.body_shape.clone(),
                        wearables: definition
                            .wearables
                            .iter()
                            .map(|(urn, wearable)| (wearable.category.slot, urn.to_string()))
                            .collect(),
                        hidden: hidden_slots(&definition.hides),
                    }
                }),
                emote: emote.map(|(active, _)| emote_state(active, active.source())),
                upper_body_emote: emote
                    .and_then(|(_, masked)| masked.active_request())
                    .map(|playback| emote_state(playback, ActiveEmoteSource::TriggeredEmote)),
            },
        ));
    }

    reports.sort_by_key(|(entity, report)| (report.kind, report.address, *entity));
    input.reply_ok(
        Value::Array(reports.iter().map(|(_, report)| report.to_json()).collect()).to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADDRESS: &str = "0x00000000000000000000000000000000000000a1";

    fn report() -> AvatarReport {
        AvatarReport {
            kind: Kind::OtherPlayer,
            address: ADDRESS.as_h160(),
            selected: Selected {
                body_shape: Some("urn:decentraland:off-chain:base-avatars:BaseFemale".to_owned()),
                wearables: vec!["urn:decentraland:off-chain:base-avatars:f_eyes_00".to_owned()],
                emotes: vec![String::new(), "wave".to_owned()],
                force_render: vec!["hair".to_owned()],
                from_scene: false,
            },
            rendered: Some(Rendered {
                processed: true,
                failed: false,
                body_shape: "urn:decentraland:off-chain:base-avatars:basefemale".to_owned(),
                wearables: vec![(
                    "eyes",
                    "urn:decentraland:off-chain:base-avatars:f_eyes_00".to_owned(),
                )],
                hidden: vec!["feet", "hat"],
            }),
            emote: Some(EmoteState {
                urn: "idle_male".to_owned(),
                source: "velocitySelected",
                repeat: true,
                finished: false,
                mask: "fullBody",
            }),
            upper_body_emote: None,
        }
    }

    #[test]
    fn a_report_is_camel_case_json() {
        assert_eq!(
            report().to_json(),
            json!({
                "kind": "otherPlayer",
                "address": ADDRESS,
                "selected": {
                    "bodyShape": "urn:decentraland:off-chain:base-avatars:BaseFemale",
                    "wearables": ["urn:decentraland:off-chain:base-avatars:f_eyes_00"],
                    "emotes": ["", "wave"],
                    "forceRender": ["hair"],
                    "fromScene": false,
                },
                "rendered": {
                    "processed": true,
                    "failed": false,
                    "bodyShape": "urn:decentraland:off-chain:base-avatars:basefemale",
                    "wearables": [{
                        "category": "eyes",
                        "urn": "urn:decentraland:off-chain:base-avatars:f_eyes_00",
                    }],
                    "hidden": ["feet", "hat"],
                },
                "emote": {
                    "urn": "idle_male",
                    "source": "velocitySelected",
                    "repeat": true,
                    "finished": false,
                    "mask": "fullBody",
                },
                "upperBodyEmote": null,
            })
        );
    }

    #[test]
    fn an_avatar_not_built_yet_has_no_rendered_part() {
        let report = AvatarReport {
            kind: Kind::SceneAvatar,
            address: None,
            rendered: None,
            emote: None,
            ..report()
        };
        let json = report.to_json();
        assert_eq!(json["kind"], "sceneAvatar");
        assert_eq!(json["address"], Value::Null);
        assert_eq!(json["rendered"], Value::Null);
        assert_eq!(json["emote"], Value::Null);
        assert_eq!(json["selected"]["wearables"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn hidden_categories_list_in_a_fixed_order() {
        assert_eq!(
            hidden_slots(&[
                WearableCategory::HAT,
                WearableCategory::FEET,
                WearableCategory::BODY_SHAPE,
            ]),
            ["body_shape", "feet", "hat"]
        );
        assert!(hidden_slots(&[]).is_empty());
    }

    #[test]
    fn the_target_is_the_player_all_or_an_address() {
        assert_eq!(Target::parse(None), Ok(Target::Player));
        assert_eq!(Target::parse(Some("all")), Ok(Target::All));
        let address = ADDRESS.as_h160().unwrap();
        assert_eq!(Target::parse(Some(ADDRESS)), Ok(Target::Address(address)));
        // an address, with or without its prefix and in any case
        assert_eq!(
            Target::parse(Some("00000000000000000000000000000000000000A1")),
            Ok(Target::Address(address))
        );
        assert!(Target::parse(Some("0x1234")).is_err());
        assert!(Target::parse(Some("everyone")).is_err());
    }

    #[test]
    fn a_target_selects_the_avatars_it_names() {
        let address = ADDRESS.as_h160();
        let other = "0x00000000000000000000000000000000000000b2".as_h160();
        let by_address = Target::Address(address.unwrap());

        assert!(Target::Player.selects(Kind::Player, address));
        assert!(!Target::Player.selects(Kind::OtherPlayer, address));
        assert!(!Target::Player.selects(Kind::Preview, None));

        assert!(Target::All.selects(Kind::SceneAvatar, None));
        assert!(Target::All.selects(Kind::Preview, None));

        assert!(by_address.selects(Kind::OtherPlayer, address));
        // the local player is found by address too
        assert!(by_address.selects(Kind::Player, address));
        assert!(!by_address.selects(Kind::OtherPlayer, other));
        assert!(!by_address.selects(Kind::SceneAvatar, None));
    }
}
