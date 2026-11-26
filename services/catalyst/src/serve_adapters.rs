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

    fn marks<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        chain_id: i64,
        land: &'a str,
        estate: &'a str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> futures::future::BoxFuture<
        'a,
        Result<explore::api::map::build::Marks, explore::api::map::LandMarketError>,
    > {
        use explore::api::map::build;
        use shop::market::land_map;
        Box::pin(async move {
            let land_map::LandMarketMarks { ready, marks } =
                land_map::land_market_marks(&self.0, conn, chain_id, land, estate, now)
                    .await
                    .map_err(|e| match e {
                        land_map::LandMarketError::Unready(source, why) => {
                            build::LandMarketError::Unready(source, why)
                        }
                        land_map::LandMarketError::Failed(e) => build::LandMarketError::Failed(e),
                    })?;
            Ok(build::Marks { ready, marks })
        })
    }
}

/// The analytics API as the credits revenue events' sink
/// (`shop::credits::events`), under credits-server's context.
pub(super) struct CreditsRevenueEvents(social::analytics::Analytics);

impl CreditsRevenueEvents {
    pub(super) fn new(analytics: &social::analytics::Analytics) -> Self {
        Self(analytics.with_context(shop::credits::events::ANALYTICS_CONTEXT))
    }
}

impl shop::credits::events::RevenueEvents for CreditsRevenueEvents {
    fn fire(&self, event: &str, properties: serde_json::Value) {
        self.0.fire_event(event, properties);
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

#[cfg(test)]
mod credits_revenue_events_tests {
    use super::*;

    /// The credits revenue events reach the analytics API under
    /// credits-server's context and Social's events under its own, through
    /// one client: the two views share its transport.
    #[tokio::test]
    async fn credits_revenue_events_post_under_credits_servers_context() {
        let (sent, mut received) = tokio::sync::mpsc::unbounded_channel();
        let app = axum::Router::new().route(
            "/events",
            axum::routing::post(move |body: axum::body::Bytes| {
                let sent = sent.clone();
                async move {
                    let _ = sent.send(serde_json::from_slice::<serde_json::Value>(&body).unwrap());
                    axum::http::StatusCode::OK
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let analytics = social::analytics::Analytics::new(
            &crate::http::Outbound::for_tests(),
            social::analytics::Settings {
                context: Some("social".into()),
                api_url: Some(format!("http://{address}/events")),
                api_token: Some("token".into()),
                environment: Some("test".into()),
                request_timeout_ms: None,
            },
        )
        .unwrap()
        .unwrap();
        let events = CreditsRevenueEvents::new(&analytics);
        assert!(
            events.0.shares_transport_with(&analytics),
            "one transport for both views"
        );
        shop::credits::events::RevenueEvents::fire(
            &events,
            shop::credits::events::STRIPE_CREDIT_GRANTED,
            serde_json::json!({ "orderId": "ord_1" }),
        );
        analytics.fire_event("social_event", serde_json::json!({ "id": 1 }));
        let mut posted = Vec::new();
        for _ in 0..2 {
            posted.push(
                tokio::time::timeout(std::time::Duration::from_secs(5), received.recv())
                    .await
                    .unwrap()
                    .unwrap(),
            );
        }
        posted.sort_by_key(|p| p["event"].as_str().unwrap_or_default().to_string());
        assert_eq!(
            (posted[0]["event"].clone(), posted[0]["context"].clone()),
            (
                serde_json::json!("social_event"),
                serde_json::json!("social")
            )
        );
        assert_eq!(
            (posted[1]["event"].clone(), posted[1]["context"].clone()),
            (
                serde_json::json!("stripe_credit_granted"),
                serde_json::json!("credits-server")
            )
        );
        assert_eq!(posted[1]["body"]["orderId"], "ord_1");
    }
}
