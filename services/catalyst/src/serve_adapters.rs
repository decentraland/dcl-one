//! What `serve` hands Shop from Social and from Builder, Shop's answer Comms
//! asks for, the live participants Comms gives Explore, and Shop's LAND
//! listings Explore's map grid reads: those crates build without each other,
//! so these wrappers connect them here.

use std::sync::Arc;

use crate::shared::identity::Address;
use crate::shared::notifications::SendFailure;
use crate::{builder, comms, explore, shop, social};

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
        conn: &'a mut sqlx::PgConnection,
        chain_id: i64,
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
            let m = land_map::land_market(&self.0, conn, chain_id, land, estate, now)
                .await
                .map_err(|e| match e {
                    land_map::LandMarketError::Unready(source, why) => {
                        build::LandMarketError::Unready(source, why)
                    }
                    land_map::LandMarketError::Failed(e) => build::LandMarketError::Failed(e),
                })?;
            // Every field named on both sides: one added to either fails to build.
            let land_map::LandMarket {
                orders,
                trades,
                rentals,
                data,
            } = m;
            Ok(build::LandMarketSnapshot {
                orders: orders
                    .into_iter()
                    .map(|o| {
                        let land_map::LandOrder {
                            contract_address,
                            token_id,
                            open,
                            seller,
                            price,
                            expires_at_ms,
                            created_at,
                            updated_at,
                        } = o;
                        build::LandOrder {
                            contract_address,
                            token_id,
                            open,
                            seller,
                            price,
                            expires_at_ms,
                            created_at,
                            updated_at,
                        }
                    })
                    .collect(),
                trades: trades
                    .into_iter()
                    .map(|t| {
                        let land_map::LandTrade {
                            id,
                            contract_address,
                            token_id,
                            signer,
                            price,
                            created_at_ms,
                        } = t;
                        build::LandTrade {
                            id,
                            contract_address,
                            token_id,
                            signer,
                            price,
                            created_at_ms,
                        }
                    })
                    .collect(),
                rentals: rentals
                    .into_iter()
                    .map(|r| {
                        let land_map::LandRental {
                            contract_address,
                            token_id,
                            expiration_ms,
                            updated_at_ms,
                            periods,
                        } = r;
                        build::LandRental {
                            contract_address,
                            token_id,
                            expiration_ms,
                            updated_at_ms,
                            periods,
                        }
                    })
                    .collect(),
                data: data
                    .into_iter()
                    .map(|d| {
                        let land_map::LandData {
                            contract_address,
                            token_id,
                            name,
                            updated_at,
                        } = d;
                        build::LandData {
                            contract_address,
                            token_id,
                            name,
                            updated_at,
                        }
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

/// Builder's publication fee as what a publication paid with Shop credits
/// costs and whose it is.
pub(super) struct BuilderPublications(pub(super) builder::SharedState);

#[async_trait::async_trait]
impl shop::credits::publication::Publications for BuilderPublications {
    async fn quote(
        &self,
        signer: &crate::shared::signed_fetch::Signer,
        data: &[u8],
    ) -> Result<shop::credits::publication::PublicationQuote, shop::credits::errors::ApiError> {
        use shop::credits::errors::ApiError;
        let fee = builder::publication_fee::for_call(&self.0, signer, data)
            .await
            .map_err(|refusal| {
                let message = refusal.to_string();
                match refusal.status() {
                    400 => ApiError::bad_request(message),
                    401 | 403 => ApiError::forbidden(message),
                    404 => ApiError::not_found(message),
                    409 | 423 => ApiError::conflict(message),
                    429 | 503 => ApiError::service_unavailable(message),
                    _ => ApiError::Internal(message),
                }
            })?;
        let contract = shop::EvmAddress::parse(&fee.contract)
            .map_err(|e| ApiError::Internal(format!("the collection's contract: {e}")))?;
        Ok(shop::credits::publication::PublicationQuote {
            contract,
            usd_cents: fee.usd_cents,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alloy_sol_types::SolCall;

    use super::BuilderPublications;
    use crate::shared::signed_fetch::test_support::signer;
    use crate::shop::credits::errors::ApiError;
    use crate::shop::credits::publication::{Publications, createCollectionCall};
    use crate::{builder, http, storage};

    const CREATOR: &str = "0x00000000000000000000000000000000000000c1";

    async fn publications(pool: sqlx::PgPool) -> BuilderPublications {
        let dir = std::env::temp_dir().join(format!(
            "catalyst-publication-fee-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let storage = Arc::new(storage::Storage::from_config(&dir).await.unwrap());
        BuilderPublications(Arc::new(
            builder::State::new(
                pool,
                storage.clone(),
                storage.denylist().clone(),
                http::Outbound::for_tests(),
                None,
                None,
                builder::Settings::default(),
            )
            .unwrap(),
        ))
    }

    #[tokio::test]
    async fn shop_is_refused_a_publication_the_builder_refuses_with_the_builders_reason() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let builder = publications(pool).await;

        let refused = builder
            .quote(&signer(CREATOR), &[0u8; 7])
            .await
            .expect_err("bytes that are no createCollection arguments");
        assert!(matches!(refused, ApiError::BadRequest(_)), "{refused:?}");

        let network = builder::network::POLYGON;
        let unknown = createCollectionCall {
            forwarder: network.forwarder.parse().unwrap(),
            factory: network.collection_factory.parse().unwrap(),
            salt: Default::default(),
            name: "No such draft".into(),
            symbol: "DCL-NSCHDRFT".into(),
            baseURI: network.base_uri.into(),
            creator: CREATOR.parse().unwrap(),
            items: Vec::new(),
        };
        let refused = builder
            .quote(&signer(CREATOR), &unknown.abi_encode()[4..])
            .await
            .expect_err("a publication without items");
        assert!(matches!(refused, ApiError::BadRequest(_)), "{refused:?}");

        let mut one_item = unknown.clone();
        one_item.items = vec![crate::shop::credits::publication::PublicationItem {
            rarity: "common".into(),
            price: Default::default(),
            beneficiary: Default::default(),
            metadata: "1:w:Hat::hat:BaseMale".into(),
        }];
        let refused = builder
            .quote(&signer(CREATOR), &one_item.abi_encode()[4..])
            .await
            .expect_err("a publication of no stored draft");
        assert!(matches!(refused, ApiError::NotFound(_)), "{refused:?}");
    }
}
