use bevy::prelude::*;
use common::structs::{default_home_realm, home_account, AppConfig, PinnedHome};
use system_bridge::{HomeScene, SystemApi};
use wallet::Wallet;

mod diagnostics;

pub(crate) struct HomePlugin;

impl Plugin for HomePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_home_scene)
            .add_plugins(diagnostics::HomeDebugPanelPlugin);
    }
}

/// The key of the signed-in account's home. A guest with no account (`Wallet::is_guest`) has a
/// new address each session, so it has none.
fn signed_in_account(wallet: &Wallet) -> Option<String> {
    (!wallet.is_guest())
        .then(|| wallet.address())
        .flatten()
        .map(|address| home_account(&address))
}

/// The home belongs to the signed-in account: read, pinned and cleared for it alone. With no
/// account it is the startup account's, and nothing is written.
fn handle_home_scene(
    mut ev: EventReader<SystemApi>,
    mut config: ResMut<AppConfig>,
    wallet: Res<Wallet>,
) {
    for ev in ev.read() {
        if !matches!(
            ev,
            SystemApi::GetHomeScene(_) | SystemApi::SetHomeScene(_) | SystemApi::ClearHomeScene
        ) {
            continue;
        }
        let account = signed_in_account(&wallet);
        // adopting a home on a read is saved without waking the systems that watch the config
        if apply(ev, config.bypass_change_detection(), account.as_deref()) {
            if !matches!(ev, SystemApi::GetHomeScene(_)) {
                config.set_changed();
            }
            platform::write_config_file(&*config);
        }
    }
}

/// True when the config changed.
fn apply(ev: &SystemApi, config: &mut AppConfig, account: Option<&str>) -> bool {
    let mut changed = account.is_some_and(|account| config.adopt_home(account));
    match (ev, account) {
        (SystemApi::GetHomeScene(sender), Some(account)) => {
            sender.send(home_scene(config.pinned_home(Some(account)), true));
        }
        (SystemApi::GetHomeScene(sender), None) => {
            sender.send(home_scene(config.startup_home(), false));
        }
        (SystemApi::SetHomeScene(home), Some(account)) => {
            config.homes.insert(
                account.to_owned(),
                PinnedHome {
                    realm: home.realm.clone(),
                    location: Vec2::from(&home.parcel).as_ivec2(),
                },
            );
            changed = true;
        }
        (SystemApi::ClearHomeScene, Some(account)) => {
            changed |= config.homes.remove(account).is_some();
        }
        _ => (),
    }
    changed
}

fn home_scene(home: Option<PinnedHome>, pinnable: bool) -> HomeScene {
    let pinned = home.is_some();
    let home = home.unwrap_or_else(|| PinnedHome {
        realm: default_home_realm(),
        location: IVec2::ZERO,
    });
    HomeScene {
        realm: home.realm,
        parcel: home.location.as_vec2().into(),
        pinned,
        pinnable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::rpc::RpcResultSender;

    fn read(config: &mut AppConfig, account: Option<&str>) -> (Option<(String, IVec2)>, bool) {
        let (sender, mut receiver) = RpcResultSender::channel();
        let changed = apply(&SystemApi::GetHomeScene(sender), config, account);
        let home = receiver.try_recv().unwrap();
        let pinned = home
            .pinned
            .then(|| (home.realm, Vec2::from(&home.parcel).as_ivec2()));
        (pinned, changed)
    }

    fn set(realm: &str, x: i32, y: i32) -> SystemApi {
        SystemApi::SetHomeScene(HomeScene {
            realm: realm.to_owned(),
            parcel: Vec2::new(x as f32, y as f32).into(),
            pinned: true,
            pinnable: true,
        })
    }

    #[test]
    fn before_sign_in_the_startup_home_is_read_and_nothing_is_written() {
        let mut config = AppConfig::default();
        config.homes.insert(
            "0xa".into(),
            PinnedHome {
                realm: "realm-a".into(),
                location: IVec2::new(3, 4),
            },
        );
        config.home_last_account = Some("0xa".into());

        assert_eq!(
            read(&mut config, None),
            (Some(("realm-a".to_owned(), IVec2::new(3, 4))), false)
        );
        assert!(!apply(&set("realm-x", 1, 1), &mut config, None));
        assert!(!apply(&SystemApi::ClearHomeScene, &mut config, None));
        assert_eq!(config.homes.len(), 1);
        assert_eq!(config.home_last_account.as_deref(), Some("0xa"));
    }

    #[test]
    fn a_guest_with_no_account_is_not_a_signed_in_account() {
        let mut wallet = Wallet::default();
        assert_eq!(signed_in_account(&wallet), None);

        wallet.finalize_as_guest();
        assert!(wallet.address().is_some());
        assert_eq!(signed_in_account(&wallet), None);

        for guest_account in [true, false] {
            let root = wallet::local_guest::new_root_key();
            let (ephemeral, auth) = wallet::local_guest::sign_identity(&root, 0).unwrap();
            wallet.finalize(root.address(), ephemeral, auth, guest_account);
            assert_eq!(
                signed_in_account(&wallet),
                Some(home_account(&root.address()))
            );
        }

        wallet.disconnect();
        assert_eq!(signed_in_account(&wallet), None);
    }

    #[test]
    fn a_home_kept_for_no_account_outlives_a_guest_and_goes_to_the_next_account() {
        let mut config = AppConfig {
            home_realm: Some("realm-old".into()),
            home_location: Some(IVec2::new(7, 8)),
            ..Default::default()
        };

        let (sender, mut receiver) = RpcResultSender::channel();
        assert!(!apply(&SystemApi::GetHomeScene(sender), &mut config, None));
        let home = receiver.try_recv().unwrap();
        assert!(home.pinned);
        assert!(!home.pinnable);
        assert!(!apply(&set("realm-x", 1, 1), &mut config, None));
        assert!(!apply(&SystemApi::ClearHomeScene, &mut config, None));
        assert_eq!(config.home_realm.as_deref(), Some("realm-old"));
        assert_eq!(config.home_location, Some(IVec2::new(7, 8)));
        assert!(config.homes.is_empty());
        assert_eq!(config.home_last_account, None);

        let (sender, mut receiver) = RpcResultSender::channel();
        assert!(apply(
            &SystemApi::GetHomeScene(sender),
            &mut config,
            Some("0xa")
        ));
        let home = receiver.try_recv().unwrap();
        assert!(home.pinnable);
        assert_eq!(home.realm, "realm-old");
        assert_eq!(config.home_realm, None);
        assert_eq!(config.homes["0xa"].location, IVec2::new(7, 8));
    }

    #[test]
    fn the_signed_in_account_reads_sets_and_clears_only_its_own_home() {
        let mut config = AppConfig::default();
        assert!(apply(&set("realm-a", 3, 4), &mut config, Some("0xa")));

        assert_eq!(read(&mut config, Some("0xb")), (None, true));
        assert!(apply(&set("realm-b", -5, 6), &mut config, Some("0xb")));
        assert_eq!(
            read(&mut config, Some("0xb")),
            (Some(("realm-b".to_owned(), IVec2::new(-5, 6))), false)
        );

        assert!(apply(&SystemApi::ClearHomeScene, &mut config, Some("0xb")));
        assert!(!apply(&SystemApi::ClearHomeScene, &mut config, Some("0xb")));
        assert_eq!(read(&mut config, Some("0xb")), (None, false));
        assert_eq!(
            read(&mut config, Some("0xa")).0,
            Some(("realm-a".to_owned(), IVec2::new(3, 4)))
        );
    }
}
