//! `/wear`: put any wearable on the local player, for comparing how wearables render. Only this
//! client's rendering changes: the profile, the passport and what other players see stay as they
//! are. A preview-mode command, registered with the other avatar debug tools (`debug_avatars`).
//!
//! The forced wearables lead the player's shape, so each wins its category over the shape's own
//! (the renderer keeps the first wearable of each category). A scene that replaces the player's
//! avatar shape hides them while the player stands in it.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use collectibles::{
    wearables::{Wearable, WearableUrn},
    CollectibleError, CollectibleManager,
};
use common::structs::PrimaryUser;
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use serde_json::json;

use crate::{AvatarLoadFailures, AvatarShape};

pub(super) struct DebugWearPlugin;

impl Plugin for DebugWearPlugin {
    fn build(&self, app: &mut App) {
        app.add_preview_console_command::<WearCommand, _>(wear_command)
            .add_systems(
                Update,
                apply_forced_wearables
                    .after(wear_command)
                    .after(crate::update_base_avatar_shape)
                    .before(crate::select_avatar),
            )
            .add_debug_section(DebugSection {
                title: "Wear any wearable",
                query: "/wear",
                fields: &[("Forced wearables", "wearables"), ("State", "status")],
                actions: &[("Take off forced wearables", "/wear clear")],
                inputs: &[DebugInput {
                    label: "Wearable URNs (comma-separated)",
                    field: "",
                    command: "/wear {value}",
                    kind: DebugInputKind::Text,
                }],
            });
    }
}

/// Put wearables on the local player (`/wear <urn>[,<urn>...]`), take them all off
/// (`/wear clear`) or report them (`/wear`).
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/wear")]
struct WearCommand {
    #[arg(value_delimiter = ',')]
    urns: Vec<String>,
}

/// The local player's forced wearables, newest first.
#[derive(Component, Default, Debug)]
struct ForcedWearables {
    forced: Vec<String>,
    /// the shape's own wearables, restored by `/wear clear`
    own: Vec<String>,
    /// the list last written to the shape; any other list means the shape was replaced
    applied: Vec<String>,
}

/// `urns` go in front, in the order given; one already forced moves to its new place.
fn put_on(forced: &mut Vec<String>, urns: &[String]) {
    forced.retain(|urn| !urns.contains(urn));
    forced.splice(0..0, urns.iter().cloned());
}

/// The forced wearables, then the shape's own ones that are not forced too.
fn worn(forced: &[String], own: &[String]) -> Vec<String> {
    forced
        .iter()
        .chain(own.iter().filter(|urn| !forced.contains(urn)))
        .cloned()
        .collect()
}

/// Gives the shape its own wearables back, unless it was replaced since they were forced.
fn take_off(wearables: &mut Vec<String>, forced: &ForcedWearables) {
    if *wearables == forced.applied {
        wearables.clone_from(&forced.own);
    }
}

/// The distinct non-empty urns, or the ones that are not wearable urns.
fn parse_urns(urns: Vec<String>) -> Result<Vec<String>, Vec<String>> {
    let mut parsed: Vec<String> = Vec::new();
    let mut invalid = Vec::new();
    for urn in urns.into_iter().map(|urn| urn.trim().to_owned()) {
        if urn.is_empty() || parsed.contains(&urn) {
            continue;
        }
        if WearableUrn::new_quiet(&urn).is_ok() {
            parsed.push(urn);
        } else {
            invalid.push(urn);
        }
    }
    if invalid.is_empty() {
        Ok(parsed)
    } else {
        Err(invalid)
    }
}

/// Keeps the forced wearables in front of the player's shape, whoever replaces the shape (a
/// profile update, `/avatar_look`).
fn apply_forced_wearables(
    mut players: Query<
        (&mut ForcedWearables, &mut AvatarShape),
        (
            With<PrimaryUser>,
            Or<(Changed<ForcedWearables>, Changed<AvatarShape>)>,
        ),
    >,
) {
    for (mut forced, mut shape) in &mut players {
        let forced = forced.bypass_change_detection();
        if shape.0.wearables != forced.applied {
            forced.own.clone_from(&shape.0.wearables);
        }
        let worn = worn(&forced.forced, &forced.own);
        if shape.0.wearables != worn {
            shape.0.wearables.clone_from(&worn);
        }
        forced.applied = worn;
    }
}

fn wear_command(
    mut cmd: ConsoleCommand<WearCommand>,
    mut commands: Commands,
    mut players: Query<
        (
            Entity,
            &mut AvatarShape,
            Option<&mut ForcedWearables>,
            Option<&AvatarLoadFailures>,
        ),
        With<PrimaryUser>,
    >,
    mut wearables: CollectibleManager<Wearable>,
) {
    let Some(Ok(args)) = cmd.take() else { return };
    let Ok((entity, mut shape, mut forced, failures)) = players.single_mut() else {
        cmd.reply_failed("local player unavailable");
        return;
    };
    let mut current = forced
        .as_ref()
        .map(|f| f.forced.clone())
        .unwrap_or_default();
    if args.urns.len() == 1 && args.urns[0] == "clear" {
        if let Some(forced) = forced {
            take_off(&mut shape.0.wearables, &forced);
            commands.entity(entity).remove::<ForcedWearables>();
        }
        current.clear();
    } else if !args.urns.is_empty() {
        let urns = match parse_urns(args.urns) {
            Ok(urns) => urns,
            Err(invalid) => {
                cmd.reply_failed(format!("not wearable urns: {}", invalid.join(", ")));
                return;
            }
        };
        put_on(&mut current, &urns);
        match forced.as_mut() {
            Some(forced) => forced.forced.clone_from(&current),
            None => {
                commands.entity(entity).insert(ForcedWearables {
                    forced: current.clone(),
                    ..default()
                });
            }
        }
    }
    let body = crate::requested_body(&shape.0);
    let states: Vec<_> = current
        .iter()
        .map(|urn| (urn, state(&mut wearables, failures, urn, body.as_str())))
        .collect();
    let status = states
        .iter()
        .map(|(urn, state)| format!("{urn}: {state}"))
        .collect::<Vec<_>>()
        .join("; ");
    cmd.reply_ok(
        json!({
            "wearables": current.join(","),
            "status": status,
            "states": states.iter().map(|(urn, state)| json!({"urn": urn, "state": state})).collect::<Vec<_>>(),
        })
        .to_string(),
    );
}

/// Where a forced wearable stands: its category once loaded, else why it is not worn.
fn state(
    wearables: &mut CollectibleManager<Wearable>,
    failures: Option<&AvatarLoadFailures>,
    urn: &str,
    body: &str,
) -> String {
    let Ok(urn) = WearableUrn::new_quiet(urn) else {
        return "not a wearable urn".into();
    };
    if AvatarLoadFailures::wearable_failed(failures, &urn) {
        return "its model failed to load".into();
    }
    match wearables.get_representation(&urn, body) {
        Ok(wearable) => wearable.category.slot.to_owned(),
        Err(CollectibleError::Loading) => "loading".into(),
        Err(CollectibleError::Missing) => "not found".into(),
        Err(CollectibleError::NoRepresentation) => "no representation for this body".into(),
        Err(CollectibleError::Failed) => "failed to load".into(),
    }
}

#[cfg(test)]
mod tests {
    use dcl_component::proto_components::sdk::components::PbAvatarShape;

    use super::*;

    const HAT: &str =
        "urn:decentraland:matic:collections-v2:0x0000000000000000000000000000000000000001:0";
    const MASK: &str =
        "urn:decentraland:matic:collections-v2:0x0000000000000000000000000000000000000002:0";
    const OWN: &str = "urn:decentraland:off-chain:base-avatars:eyebrows_00";
    const LOOKED: &str = "urn:decentraland:off-chain:base-avatars:eyebrows_01";

    fn strings(urns: &[&str]) -> Vec<String> {
        urns.iter().map(|urn| urn.to_string()).collect()
    }

    fn app_with_player(own: &[&str]) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, apply_forced_wearables);
        let player = app
            .world_mut()
            .spawn((
                PrimaryUser::default(),
                AvatarShape(PbAvatarShape {
                    wearables: strings(own),
                    ..default()
                }),
            ))
            .id();
        (app, player)
    }

    fn worn_by(app: &App, player: Entity) -> Vec<String> {
        app.world()
            .get::<AvatarShape>(player)
            .unwrap()
            .0
            .wearables
            .clone()
    }

    #[test]
    fn newest_forced_wearables_lead_and_are_listed_once() {
        let mut forced = strings(&[HAT]);
        put_on(&mut forced, &strings(&[MASK]));
        assert_eq!(forced, strings(&[MASK, HAT]));
        put_on(&mut forced, &strings(&[HAT]));
        assert_eq!(forced, strings(&[HAT, MASK]));
        assert_eq!(
            worn(&forced, &strings(&[OWN, MASK])),
            strings(&[HAT, MASK, OWN])
        );
    }

    #[test]
    fn only_wearable_urns_are_accepted() {
        assert_eq!(parse_urns(strings(&[HAT, " ", HAT])), Ok(strings(&[HAT])));
        assert_eq!(
            parse_urns(strings(&[HAT, "not-an-urn"])),
            Err(strings(&["not-an-urn"]))
        );
    }

    #[test]
    fn forced_wearables_lead_the_players_shape() {
        let (mut app, player) = app_with_player(&[OWN]);
        app.world_mut().entity_mut(player).insert(ForcedWearables {
            forced: strings(&[HAT]),
            ..default()
        });
        app.update();
        assert_eq!(worn_by(&app, player), strings(&[HAT, OWN]));
        // nothing changed: the shape is left alone
        app.update();
        assert!(!app
            .world()
            .entity(player)
            .get_ref::<AvatarShape>()
            .unwrap()
            .is_changed());
    }

    #[test]
    fn a_replaced_shape_keeps_the_forced_wearables() {
        let (mut app, player) = app_with_player(&[OWN]);
        app.world_mut().entity_mut(player).insert(ForcedWearables {
            forced: strings(&[HAT]),
            ..default()
        });
        app.update();
        // a profile update or `/avatar_look` replaces the shape
        app.world_mut()
            .entity_mut(player)
            .insert(AvatarShape(PbAvatarShape {
                wearables: strings(&[LOOKED]),
                ..default()
            }));
        app.update();
        assert_eq!(worn_by(&app, player), strings(&[HAT, LOOKED]));
        let forced = app.world().get::<ForcedWearables>(player).unwrap();
        assert_eq!(forced.own, strings(&[LOOKED]));
        // `/wear clear` gives the replaced shape's wearables back
        let mut wearables = worn_by(&app, player);
        take_off(&mut wearables, forced);
        assert_eq!(wearables, strings(&[LOOKED]));
    }
}
