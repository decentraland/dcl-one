//! What `serve` hands Shop from Social, Shop's answer Comms asks for, the
//! live participants Comms gives Explore, and Shop's LAND listings Explore's
//! map grid reads: those crates build without each other, so these wrappers
//! connect them here.

use std::sync::Arc;

use crate::shared::identity::Address;
use crate::shared::notifications::SendFailure;
use crate::{comms, explore, shop, social};

/// The gatekeeper's live room members, counted by Explore's destinations feed.
pub(super) struct CommsParticipants(pub(super) comms::gatekeeper::SharedState);

impl explore::places::live::Participants for CommsParticipants {
    fn participants<'a>(
        &'a self,
        world: bool,
        key: &'a str,
    ) -> futures::future::BoxFuture<'a, Option<Vec<String>>> {
        Box::pin(self.0.live_participants(world, key))
    }
}

/// Shop's LAND and Estate orders, trades, names and rentals as the map
/// grid's [`explore::api::map::LandMarket`].
pub(super) struct ShopLandMarket(pub(super) Arc<shop::market::MarketplaceState>);

impl explore::api::map::LandMarket for ShopLandMarket {
    fn snapshot<'a>(
        &'a self,
        land: &'a str,
        estate: &'a str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> futures::future::BoxFuture<
        'a,
        Result<explore::api::map::LandMarketSnapshot, explore::api::map::LandMarketError>,
    > {
        use explore::api::map::build;
        use shop::market::land_map;
        Box::pin(async move {
            let m = land_map::land_market(&self.0, land, estate, now)
                .await
                .map_err(|e| match e {
                    land_map::LandMarketError::Unready(source, why) => {
                        build::LandMarketError::Unready(source, why)
                    }
                    land_map::LandMarketError::Failed(e) => build::LandMarketError::Failed(e),
                })?;
            Ok(build::LandMarketSnapshot {
                orders: m
                    .orders
                    .into_iter()
                    .map(|o| build::LandOrder {
                        contract_address: o.contract_address,
                        token_id: o.token_id,
                        open: o.open,
                        seller: o.seller,
                        price: o.price,
                        expires_at_ms: o.expires_at_ms,
                        created_at: o.created_at,
                        updated_at: o.updated_at,
                    })
                    .collect(),
                trades: m
                    .trades
                    .into_iter()
                    .map(|t| build::LandTrade {
                        id: t.id,
                        contract_address: t.contract_address,
                        token_id: t.token_id,
                        signer: t.signer,
                        price: t.price,
                        created_at_ms: t.created_at_ms,
                    })
                    .collect(),
                rentals: m
                    .rentals
                    .into_iter()
                    .map(|r| build::LandRental {
                        contract_address: r.contract_address,
                        token_id: r.token_id,
                        expiration_ms: r.expiration_ms,
                        updated_at_ms: r.updated_at_ms,
                        periods: r.periods,
                    })
                    .collect(),
                data: m
                    .data
                    .into_iter()
                    .map(|d| build::LandData {
                        contract_address: d.contract_address,
                        token_id: d.token_id,
                        name: d.name,
                        updated_at: d.updated_at,
                    })
                    .collect(),
            })
        })
    }
}

/// Social's SendGrid sender as the Shop deliveries worker's mailer.
pub(super) struct ShopMailer(pub(super) social::notifications::email::EmailSender);

#[async_trait::async_trait]
impl shop::subscriptions::AvailabilityMailer for ShopMailer {
    fn is_configured(&self) -> bool {
        self.0.html_configured()
    }

    async fn send_html(
        &self,
        to: &str,
        subject: &str,
        html: &str,
        text: &str,
        custom_args: &serde_json::Value,
    ) -> Result<Option<String>, SendFailure> {
        self.0.send_html(to, subject, html, text, custom_args).await
    }
}

/// Social's `/profile-images` as the faces Shop's profile responses warm.
pub(super) struct ShopProfileFaces(pub(super) social::profile_images::SharedState);

impl shop::market::ProfileFaces for ShopProfileFaces {
    fn derives_faces(&self) -> bool {
        self.0.origin.is_some() || self.0.render_queue.is_some()
    }

    fn warm(&self, entities: Vec<String>) {
        self.0.warm(entities);
    }
}

/// Shop's name owners as the gatekeeper's Admin Toolkit reader.
pub(super) struct ShopNameOwners(pub(super) Arc<shop::market::MarketplaceState>);

#[async_trait::async_trait]
impl comms::interfaces::NameOwnerReader for ShopNameOwners {
    async fn name_owner(&self, name: &str) -> Result<Option<Address>, String> {
        self.0.name_owner(name).await
    }
}
