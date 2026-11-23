//! The Home section of the runtime debug panel.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::structs::{AppConfig, PinnedHome};
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use serde_json::{json, Value};
use system_bridge::SystemApi;
use wallet::Wallet;

use super::signed_in_account;

pub(super) struct HomeDebugPanelPlugin;

impl Plugin for HomeDebugPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_console_command::<HomeState, _>(home_state)
            .add_console_command::<HomeClear, _>(home_clear)
            .add_debug_section(DebugSection {
                title: "Home",
                query: "/home_state",
                fields: &[
                    ("Signed-in account", "account"),
                    ("Its home (realm and parcel)", "pinned"),
                    ("Home at startup (realm and parcel)", "startup"),
                    ("Account a home was last read for", "lastAccount"),
                    ("Home kept for no account, not yet adopted", "unadopted"),
                ],
                actions: &[("Clear home", "/home_clear")],
                inputs: &[],
            });
    }
}

/// Read the signed-in account's home and the home that applies at startup, as JSON.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/home_state")]
struct HomeState;

fn home_state(mut input: ConsoleCommand<HomeState>, config: Res<AppConfig>, wallet: Res<Wallet>) {
    if input.take().is_none() {
        return;
    }
    input.reply_ok(state(&config, signed_in_account(&wallet).as_deref()).to_string());
}

fn state(config: &AppConfig, account: Option<&str>) -> Value {
    let describe =
        |home: PinnedHome| format!("{} {},{}", home.realm, home.location.x, home.location.y);
    json!({
        "account": account,
        "pinned": account.and_then(|account| config.pinned_home(Some(account))).map(describe),
        "startup": config.startup_home().map(describe),
        "lastAccount": config.home_last_account,
        "unadopted": config.home_realm.is_some() || config.home_location.is_some(),
        "unavailable": {
            "account": "No account is signed in.",
            "pinned": if account.is_some() {
                "This account has no home."
            } else {
                "No account is signed in."
            },
            "startup": "No home applies at startup: the default realm, on 0,0.",
            "lastAccount": "No home was read for an account yet.",
        },
    })
}

/// Take the signed-in account's home back. The HUD shows it until it reads the home again.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/home_clear")]
struct HomeClear;

fn home_clear(
    mut input: ConsoleCommand<HomeClear>,
    config: Res<AppConfig>,
    wallet: Res<Wallet>,
    mut events: EventWriter<SystemApi>,
) {
    if input.take().is_none() {
        return;
    }
    let Some(account) = signed_in_account(&wallet) else {
        input.reply_failed("no account is signed in");
        return;
    };
    if config.pinned_home(Some(&account)).is_none() {
        input.reply_failed("this account has no home");
        return;
    }
    events.write(SystemApi::ClearHomeScene);
    input.reply_ok("cleared");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home(realm: &str, x: i32, y: i32) -> PinnedHome {
        PinnedHome {
            realm: realm.to_owned(),
            location: IVec2::new(x, y),
        }
    }

    #[test]
    fn signed_out_it_reports_the_startup_home_and_no_account() {
        let mut config = AppConfig::default();
        config.homes.insert("0xa".into(), home("realm-a", 3, 4));
        config.home_last_account = Some("0xa".into());

        let state = state(&config, None);
        assert_eq!(state["account"], Value::Null);
        assert_eq!(state["pinned"], Value::Null);
        assert_eq!(state["startup"], "realm-a 3,4");
        assert_eq!(state["lastAccount"], "0xa");
        assert_eq!(state["unadopted"], false);
        assert_eq!(state["unavailable"]["pinned"], "No account is signed in.");
    }

    #[test]
    fn signed_in_it_reports_that_account_s_home_alone() {
        let mut config = AppConfig::default();
        config.homes.insert("0xa".into(), home("realm-a", 3, 4));

        assert_eq!(state(&config, Some("0xa"))["pinned"], "realm-a 3,4");
        let other = state(&config, Some("0xb"));
        assert_eq!(other["account"], "0xb");
        assert_eq!(other["pinned"], Value::Null);
        assert_eq!(other["unavailable"]["pinned"], "This account has no home.");
    }

    #[test]
    fn a_home_kept_for_no_account_is_reported_and_left_unadopted() {
        let config = AppConfig {
            home_realm: Some("realm-old".into()),
            home_location: Some(IVec2::new(7, 8)),
            ..Default::default()
        };

        let state = state(&config, Some("0xa"));
        assert_eq!(state["unadopted"], true);
        assert_eq!(state["pinned"], "realm-old 7,8");
        assert_eq!(state["startup"], "realm-old 7,8");
        assert!(config.homes.is_empty());
        assert_eq!(config.home_realm.as_deref(), Some("realm-old"));
        assert_eq!(config.home_last_account, None);
    }
}
