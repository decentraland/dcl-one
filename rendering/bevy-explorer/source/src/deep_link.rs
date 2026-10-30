//! `/open_deep_link`, natively: a `decentraland://` place link applied to the running client, so
//! an end-to-end test can send the client somewhere the way a link from the browser or the
//! launcher does (docs/guidelines/running.md, "Test commands").
//!
//! No running client receives a place link today. A link reaches a client as its `--deep-link`
//! at launch (main.rs), where `realm`, `position` and `dclenv` become launch options. The only
//! link a running client reads is the sign-in one, from the launcher's bridge file
//! (`platform::deeplink::await_signin`, which leaves any other link there); where the client is
//! the scheme handler, the OS starts a new client for a place link. So there is no hand-over to
//! exercise: the command parses the link as the launch does
//! (`platform::deeplink::parse_place_link`) and sends the player where its launch options would
//! have, through the same teleport the HUD and scenes use. Sign-in links are not handled.
//!
//! The web has no `decentraland://` links: a page's entry url carries `realm` and `position`
//! itself, so there the command does not exist (`/teleport x y realm` goes the same way).

use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use console::DoAddConsoleCommand;

pub struct DeepLinkPlugin;

impl Plugin for DeepLinkPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(not(target_arch = "wasm32"))]
        app.add_console_command::<place::OpenDeepLinkCommand, _>(place::open_deep_link_cmd);
        #[cfg(target_arch = "wasm32")]
        let _ = app;
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod place {
    use std::str::FromStr;

    use bevy::prelude::*;
    use bevy_console::ConsoleCommand;
    use common::{
        rpc::{RpcCall, RpcCallEvent, RpcResultSender},
        structs::IVec2Arg,
    };
    use console::PendingConsoleResponses;

    /// Where a place link sends the player: a realm to change to, a parcel to land on, or both
    /// (the parcel then belongs to that realm).
    #[derive(Debug, PartialEq)]
    pub struct Destination {
        realm: Option<String>,
        parcel: Option<IVec2>,
    }

    impl Destination {
        /// What the command replies once it is applied, as the link named it.
        fn applied(&self) -> serde_json::Value {
            serde_json::json!({
                "realm": self.realm,
                "parcel": self.parcel.map(|parcel| [parcel.x, parcel.y]),
            })
        }
    }

    /// A place link's destination, checked as the launch checks it: the link must be a
    /// `decentraland://` one, its `dclenv` known, its `position` a parcel. The base domain is
    /// fixed once the client starts (`common::base_domain`), so a `dclenv` other than the running
    /// one cannot be applied and is refused rather than dropped. A link that names neither realm
    /// nor position has nowhere to send the player, which a test that mistyped a key should hear.
    pub fn destination(link: &str, base_domain: &str) -> Result<Destination, String> {
        let place = platform::deeplink::parse_place_link(link).map_err(|e| e.to_string())?;
        if let Some(link_domain) = &place.base_domain {
            if link_domain != base_domain {
                return Err(format!(
                    "the link is for {link_domain}, but this client runs against {base_domain} \
                     (fixed when the client starts)"
                ));
            }
        }
        let parcel = place
            .position
            .filter(|position| !position.is_empty())
            .map(|position| {
                IVec2Arg::from_str(&position)
                    .map(|parcel| parcel.0)
                    .map_err(|e| format!("position {position}: {e}"))
            })
            .transpose()?;
        let realm = place.realm.filter(|realm| !realm.is_empty());
        if realm.is_none() && parcel.is_none() {
            return Err("the link names no realm or position".to_owned());
        }
        Ok(Destination { realm, parcel })
    }

    /// Apply a `decentraland://` place link to this client as a launch with it would: `realm`
    /// changes realm, `position` is the parcel to land on (in that realm when both are given),
    /// and `dclenv` must be the base domain this client runs against. Answers once the realm is
    /// set, or the player has teleported; arrival in the world is `/world_state`'s `inWorld`
    #[derive(clap::Parser, ConsoleCommand)]
    #[command(name = "/open_deep_link")]
    pub struct OpenDeepLinkCommand {
        /// the link, e.g. `decentraland://open?realm=<world or url>&position=<x>,<y>`
        url: String,
    }

    pub fn open_deep_link_cmd(
        mut input: ConsoleCommand<OpenDeepLinkCommand>,
        mut commands: Commands,
        mut pending: ResMut<PendingConsoleResponses>,
    ) {
        let Some(Ok(command)) = input.take() else {
            return;
        };
        let destination = match destination(&command.url, common::base_domain::get()) {
            Ok(destination) => destination,
            Err(e) => {
                input.reply_failed(e);
                return;
            }
        };
        // the teleport the HUD and scenes use: with a realm it is a realm change (a full
        // reconnect) landing on the parcel, or on the realm's own spawn without one
        let (response, rx) = RpcResultSender::channel();
        commands.send_event(RpcCallEvent::engine(RpcCall::TeleportPlayer {
            to: destination.parcel,
            realm: destination.realm.clone(),
            response,
        }));
        let applied = destination.applied().to_string();
        pending.push_receiver(
            rx,
            move |result| result.map(|()| applied.clone()),
            input.take_responder(),
        );
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const ORG: &str = "decentraland.org";

        #[test]
        fn a_link_with_a_realm_and_a_position_names_both() {
            let link = "decentraland://open?realm=http%3A%2F%2F127.0.0.1%3A8000&position=-3,25";
            let destination = destination(link, ORG).unwrap();
            assert_eq!(
                destination,
                Destination {
                    realm: Some("http://127.0.0.1:8000".to_owned()),
                    parcel: Some(IVec2::new(-3, 25)),
                }
            );
            assert_eq!(
                destination.applied(),
                serde_json::json!({"realm": "http://127.0.0.1:8000", "parcel": [-3, 25]})
            );
        }

        #[test]
        fn a_realm_alone_or_a_position_alone_is_enough() {
            assert_eq!(
                destination("decentraland://open?realm=mymap.dcl.eth", ORG).unwrap(),
                Destination {
                    realm: Some("mymap.dcl.eth".to_owned()),
                    parcel: None,
                }
            );
            let parcel = destination("decentraland://?position=139,-40", ORG).unwrap();
            assert_eq!(
                parcel,
                Destination {
                    realm: None,
                    parcel: Some(IVec2::new(139, -40)),
                }
            );
            assert_eq!(
                parcel.applied(),
                serde_json::json!({"realm": null, "parcel": [139, -40]})
            );
        }

        #[test]
        fn a_link_without_a_question_mark_is_all_query() {
            assert_eq!(
                destination("decentraland://realm=mymap.dcl.eth&position=1,2", ORG).unwrap(),
                Destination {
                    realm: Some("mymap.dcl.eth".to_owned()),
                    parcel: Some(IVec2::new(1, 2)),
                }
            );
        }

        #[test]
        fn the_environment_must_be_the_one_running() {
            assert!(destination("decentraland://open?position=1,2&dclenv=org", ORG).is_ok());
            let wrong = destination(
                "decentraland://open?position=1,2&dclenv=zone",
                "decentraland.org",
            )
            .unwrap_err();
            assert!(wrong.contains("decentraland.zone"), "{wrong}");
            assert!(wrong.contains("decentraland.org"), "{wrong}");
            assert!(destination(
                "decentraland://open?position=1,2&dclenv=zone",
                "decentraland.zone"
            )
            .is_ok());
            // the parse's own refusal passes through
            let unknown = destination("decentraland://open?dclenv=prod", ORG).unwrap_err();
            assert!(unknown.contains("prod"), "{unknown}");
        }

        #[test]
        fn what_is_not_a_place_is_refused() {
            let other = destination("https://decentraland.org/?position=1,2", ORG).unwrap_err();
            assert!(other.contains("decentraland://"), "{other}");
            let bad_position = destination("decentraland://open?position=north", ORG).unwrap_err();
            assert!(bad_position.contains("position north"), "{bad_position}");
            // a link that names nothing, as a sign-in link or a mistyped key does
            for link in [
                "decentraland://open",
                "decentraland://open?dclenv=org",
                "decentraland://open?signin=id-1&bridgeOnly=true&authRequestId=req-1",
                "decentraland://open?realms=mymap.dcl.eth",
                "decentraland://open?realm=&position=",
            ] {
                let error = destination(link, ORG).unwrap_err();
                assert!(error.contains("no realm or position"), "{link}: {error}");
            }
        }
    }
}
