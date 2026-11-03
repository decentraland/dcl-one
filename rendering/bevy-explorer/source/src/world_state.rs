//! Whether the player is in the world, judged one way for everything that asks: `/world_state`
//! (end-to-end tests, in the client and in the headless client) and the benchmark autopilot
//! (src/autopilot.rs).

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::structs::{CurrentRealm, OutOfWorld, PrimaryUser, WorldHold};
use console::DoAddConsoleCommand;
use wallet::Wallet;

/// `/world_state` and the [`Arrivals`] counter it and the autopilot read. It needs no window, HUD
/// or input, so the client and the headless client add it alike (the autopilot expects it in the
/// client). It adds [`ArrivalsPlugin`] itself.
pub struct WorldStatePlugin;

impl Plugin for WorldStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ArrivalsPlugin)
            .add_console_command::<WorldStateCommand, _>(world_state_cmd);
    }
}

/// The [`Arrivals`] counter alone, for an app without the console (a test of the autopilot).
pub struct ArrivalsPlugin;

impl Plugin for ArrivalsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Arrivals>()
            // before `Update`, where `/world_state` and the autopilot read the count
            .add_systems(PreUpdate, count_arrivals);
    }
}

/// How many times the player has come into the world (its `OutOfWorld` removed). Every realm
/// change and parcel teleport takes it out of world first, so a driver reads this before one and
/// waits for it to grow: `inWorld` alone is still true for a frame or two after the request. It
/// also catches a period out of world that began and ended between two samples, which leaves no
/// trace in a query.
#[derive(Resource, Default)]
pub struct Arrivals(pub u64);

fn count_arrivals(
    mut arrivals: ResMut<Arrivals>,
    mut released: RemovedComponents<OutOfWorld>,
    player: Query<(), With<PrimaryUser>>,
) {
    arrivals.0 += released
        .read()
        .filter(|entity| player.contains(*entity))
        .count() as u64;
}

/// What the player's being in the world is judged from.
pub struct WorldState {
    pub signed_in: bool,
    pub world_held: bool,
    pub realm_connected: bool,
    /// `None` while there is no player entity
    pub out_of_world: Option<bool>,
}

impl WorldState {
    /// Signed in, no world hold, the realm connected and the player not out of world. It has no
    /// history: the autopilot also needs [`Arrivals`] above zero, to tell arriving from never
    /// leaving, and a driver of `/world_state` reads the count for the same reason.
    pub fn in_world(&self) -> bool {
        self.signed_in
            && !self.world_held
            && self.realm_connected
            && self.out_of_world == Some(false)
    }
}

// --- /world_state ---

/// Report, as JSON, whether the player is in world as the benchmark autopilot judges it: signed
/// in, no world hold, the realm connected and the player not out of world. `arrivals` counts the
/// player's returns to the world: read it before a teleport or realm change and wait for it to grow
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/world_state")]
struct WorldStateCommand;

fn world_state_cmd(
    mut input: ConsoleCommand<WorldStateCommand>,
    arrivals: Res<Arrivals>,
    wallet: Res<Wallet>,
    world_hold: Option<Res<WorldHold>>,
    realm: Res<CurrentRealm>,
    player: Query<(&Transform, Has<OutOfWorld>), With<PrimaryUser>>,
) {
    if let Some(Ok(_)) = input.take() {
        let player = player.single().ok();
        let address = wallet.address();
        let state = WorldState {
            signed_in: address.is_some(),
            world_held: world_hold.is_some(),
            realm_connected: realm.connected,
            out_of_world: player.map(|(_, out)| out),
        };
        input.reply_ok(
            serde_json::json!({
                "inWorld": state.in_world(),
                "arrivals": arrivals.0,
                "signedIn": state.signed_in,
                "address": address.map(|a| format!("{a:#x}")),
                "guest": address.is_some() && wallet.is_guest_account(),
                "realm": realm.address,
                "realmConnected": state.realm_connected,
                "worldHeld": state.world_held,
                "outOfWorld": state.out_of_world,
                // the same axes as /player_position
                "position": player.map(|(t, _)| [t.translation.x, t.translation.y, -t.translation.z]),
            })
            .to_string(),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use bevy_console::{ConsoleCommandEntered, ConsoleResponder};
    use console::ConsolePlugin;

    use super::*;

    #[test]
    fn in_world_needs_every_condition() {
        let state = WorldState {
            signed_in: true,
            world_held: false,
            realm_connected: true,
            out_of_world: Some(false),
        };
        assert!(state.in_world());
        assert!(!WorldState {
            signed_in: false,
            ..state
        }
        .in_world());
        assert!(!WorldState {
            world_held: true,
            ..state
        }
        .in_world());
        assert!(!WorldState {
            realm_connected: false,
            ..state
        }
        .in_world());
        assert!(!WorldState {
            out_of_world: Some(true),
            ..state
        }
        .in_world());
        assert!(!WorldState {
            out_of_world: None,
            ..state
        }
        .in_world());
    }

    /// The command in the app the headless client has: the console without bevy_console's UI, and
    /// no window, HUD or input.
    #[test]
    fn world_state_answers_without_a_window() {
        let mut app = App::new();
        app.add_plugins((
            ConsolePlugin {
                add_bevy_console: false,
            },
            WorldStatePlugin,
        ))
        .init_resource::<Wallet>()
        .init_resource::<CurrentRealm>();
        app.world_mut().spawn((
            PrimaryUser::default(),
            Transform::from_xyz(8.0, 0.0, -24.0),
            OutOfWorld,
        ));
        // Startup registers the command with the console
        app.update();

        let answer = Arc::new(Mutex::new(None));
        let responder: ConsoleResponder = {
            let answer = answer.clone();
            Arc::new(move |result| *answer.lock().unwrap() = Some(result))
        };
        app.world_mut().send_event(ConsoleCommandEntered {
            command_name: "/world_state".to_owned(),
            args: Vec::new(),
            responder: Some(responder),
        });
        app.update();

        let reply = answer.lock().unwrap().take().expect("no reply");
        let state: serde_json::Value = serde_json::from_str(&reply.unwrap()).unwrap();
        assert_eq!(state["inWorld"], false);
        assert_eq!(state["signedIn"], false);
        assert_eq!(state["outOfWorld"], true);
        assert_eq!(state["arrivals"], 0);
        assert_eq!(state["position"], serde_json::json!([8.0, 0.0, 24.0]));
    }

    #[test]
    fn only_the_players_return_to_the_world_is_an_arrival() {
        let mut app = App::new();
        app.add_plugins(ArrivalsPlugin);
        let player = app
            .world_mut()
            .spawn((PrimaryUser::default(), OutOfWorld))
            .id();
        let other = app.world_mut().spawn(OutOfWorld).id();
        app.update();

        app.world_mut().entity_mut(other).remove::<OutOfWorld>();
        app.update();
        assert_eq!(app.world().resource::<Arrivals>().0, 0);

        // out of world and back between two frames: only the removal is left
        app.world_mut().entity_mut(player).remove::<OutOfWorld>();
        app.world_mut().entity_mut(player).insert(OutOfWorld);
        app.world_mut().entity_mut(player).remove::<OutOfWorld>();
        app.update();
        assert_eq!(app.world().resource::<Arrivals>().0, 2);
    }
}
