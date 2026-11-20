use bevy::prelude::*;
use common::structs::{default_home_realm, home_account, AppConfig, PinnedHome};
use system_bridge::{HomeScene, SystemApi};
use wallet::Wallet;

/// The home belongs to the signed-in account: read, pinned and cleared for it alone. Before
/// sign-in it is the startup account's, and nothing is written.
pub(crate) fn handle_home_scene(
    mut ev: EventReader<SystemApi>,
    mut config: ResMut<AppConfig>,
    wallet: Option<Res<Wallet>>,
) {
    for ev in ev.read() {
        if !matches!(
            ev,
            SystemApi::GetHomeScene(_) | SystemApi::SetHomeScene(_) | SystemApi::ClearHomeScene
        ) {
            continue;
        }
        let account = wallet
            .as_ref()
            .and_then(|wallet| wallet.address())
            .map(|address| home_account(&address));
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
            sender.send(home_scene(config.pinned_home(Some(account))));
        }
        (SystemApi::GetHomeScene(sender), None) => {
            sender.send(home_scene(config.startup_home()));
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

fn home_scene(home: Option<PinnedHome>) -> HomeScene {
    let pinned = home.is_some();
    let home = home.unwrap_or_else(|| PinnedHome {
        realm: default_home_realm(),
        location: IVec2::ZERO,
    });
    HomeScene {
        realm: home.realm,
        parcel: home.location.as_vec2().into(),
        pinned,
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
