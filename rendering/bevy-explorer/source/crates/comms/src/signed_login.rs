use bevy::prelude::*;
use common::structs::{AppConfig, CurrentRealm};
use ipfs::world_access::WorldCommsSecret;
use wallet::Wallet;

use crate::room_connection::{
    CredentialRequest, NeedsAdapter, RealmAdapter, RealmRoom, RoomContext,
};

pub struct SignedLoginPlugin;

impl Plugin for SignedLoginPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<StartSignedLogin>()
            .add_systems(Update, start_signed_login);
    }
}

#[derive(Event)]
pub struct StartSignedLogin {
    pub entity: Entity,
    pub address: String,
    pub context: Entity,
}

fn start_signed_login(
    mut commands: Commands,
    mut events: EventReader<StartSignedLogin>,
    realm: Res<CurrentRealm>,
    wallet: Res<Wallet>,
    config: Res<AppConfig>,
    secret: Res<WorldCommsSecret>,
) {
    for event in events.read() {
        let Ok(origin) = crate::adapter::validate_url(&realm.address) else {
            warn!("invalid signed-login realm origin");
            continue;
        };
        let mut metadata = serde_json::json!({
            "intent": "dcl:explorer:comms-handshake", "signer": "dcl:explorer",
            "isGuest": wallet.is_guest(), "origin": origin.origin().ascii_serialization(),
        });
        // A realm chooses its login endpoint. Do not let third-party realms correlate wallets
        // through the installation's persistent identifier.
        if crate::adapter::allows_device_identifier(&event.address) {
            metadata["deviceIdentifier"] = serde_json::json!(config.user_id);
        }
        if let Some(secret) = &secret.0 {
            metadata["secret"] = serde_json::json!(secret);
        }
        if let Ok(mut entity) = commands.get_entity(event.entity) {
            entity.try_insert((
                CredentialRequest {
                    method: "POST",
                    url: event.address.clone(),
                    metadata: metadata.to_string(),
                },
                NeedsAdapter,
                RoomContext(event.context),
                RealmRoom,
                RealmAdapter,
                crate::room_connection::RoomAccessScope(crate::access_status::AccessScope::Realm(
                    realm.address.clone(),
                )),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_login_endpoints_cannot_receive_the_installation_identifier() {
        for (address, allowed) in [
            ("https://decentraland.org/login", true),
            ("https://decentraland.zone/login", true),
            ("https://comms-gatekeeper.decentraland.org/login", true),
            ("https://comms-gatekeeper.decentraland.zone/login", true),
            ("https://COMMS-GATEKEEPER.DECENTRALAND.ORG/login", true),
            ("https://custom.example/login", false),
            ("https://decentraland.org.attacker.example/login", false),
            ("https://notdecentraland.org/login", false),
            ("https://decentraland.today/login", false),
            ("https://user@decentraland.org/login", false),
            ("https://decentraland.org/login#fragment", false),
            ("http://decentraland.org/login", false),
            ("wss://decentraland.org/login", false),
            ("http://localhost:8000/login", false),
        ] {
            let mut app = App::new();
            let mut realm = CurrentRealm::default();
            realm.address = "https://realm.example".into();
            app.insert_resource(realm)
                .init_resource::<Wallet>()
                .init_resource::<AppConfig>()
                .init_resource::<WorldCommsSecret>()
                .add_plugins(SignedLoginPlugin);
            let owner = app.world_mut().spawn_empty().id();
            app.world_mut().send_event(StartSignedLogin {
                entity: owner,
                address: address.into(),
                context: owner,
            });
            app.update();
            let request = app.world().get::<CredentialRequest>(owner).unwrap();
            let metadata: serde_json::Value = serde_json::from_str(&request.metadata).unwrap();
            let expected = serde_json::json!(app.world().resource::<AppConfig>().user_id);
            assert_eq!(
                metadata.get("deviceIdentifier"),
                allowed.then_some(&expected),
                "{address}"
            );
        }
    }
}
