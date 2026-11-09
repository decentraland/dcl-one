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
            "deviceIdentifier": config.user_id,
        });
        if let Some(secret) = &secret.0 {
            metadata["secret"] = serde_json::json!(secret);
        }
        if let Ok(mut entity) = commands.get_entity(event.entity) {
            entity.insert((
                CredentialRequest {
                    method: "POST",
                    url: event.address.clone(),
                    metadata: metadata.to_string(),
                },
                NeedsAdapter,
                RoomContext(event.context),
                RealmRoom,
                RealmAdapter,
                crate::room_connection::RoomAccessScope(common::access_status::AccessScope::Realm(
                    realm.address.clone(),
                )),
            ));
        }
    }
}
