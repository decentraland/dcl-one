//! What `serve` hands Shop from Social, Shop's answer Comms asks for, the
//! live participants Comms gives Explore, Shop's LAND listings Explore's
//! map grid reads and Shop's catalogue Social's first-wear notices read:
//! those crates build without each other, so these wrappers connect them
//! here.

use std::sync::Arc;

use crate::shared::identity::Address;
use crate::shared::notifications::SendFailure;
use crate::{comms, explore, shop, social};

/// Fan out durable room input. Both receivers deduplicate the webhook ID,
/// so a failed second write is safe to retry after the first has committed.
pub(super) struct CreditsRoomEvents {
    pub(super) credits: shop::credits::SharedState,
    pub(super) events_mirror: Option<String>,
    pub(super) badges: Option<Arc<dyn comms::gatekeeper::events::RoomEventStore>>,
}

#[async_trait::async_trait]
impl comms::gatekeeper::events::RoomEventStore for CreditsRoomEvents {
    async fn store_room_event(
        &self,
        event: &comms::protocol::bus::room_events::RoomEvent,
    ) -> anyhow::Result<()> {
        use comms::protocol::bus::room_events::{RoomEventKind, RoomType};
        if let Some(badges) = &self.badges {
            badges.store_room_event(event).await?;
        }
        let world = match event.room_type() {
            RoomType::World => true,
            RoomType::Scene => false,
            RoomType::Unknown => return Ok(()),
        };
        let joined = match event.kind() {
            RoomEventKind::Joined => true,
            RoomEventKind::Left => false,
            RoomEventKind::Unspecified => return Ok(()),
        };
        shop::credits::goals::rooms::store(
            &self.credits,
            shop::credits::goals::rooms::RoomEvent {
                event_id: &event.event_id,
                identity: &event.identity,
                room: &event.room,
                participant_sid: &event.participant_sid,
                joined,
                world,
                realm: &event.realm_name,
                scene_id: (!event.scene_id.is_empty()).then_some(&event.scene_id),
                event_time_ms: event.event_time_ms,
            },
            self.events_mirror.as_deref(),
        )
        .await?;
        Ok(())
    }
}

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

#[cfg(test)]
mod credits_room_event_tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use comms::interfaces::test_support::{StubDirectory, StubPolicy, StubScenes};
    use tower::ServiceExt;

    #[tokio::test]
    async fn a_failed_credits_write_retries_the_webhook_without_duplicating_badge_facts() {
        use crate::db::scratch::{OwnedDatabase, Schema};
        use comms::{
            gatekeeper, rooms,
            world_identity::{RealmId, WorldName},
        };
        let Some(db) = OwnedDatabase::new("credits_room_events", Schema::Full, 4).await else {
            return;
        };
        let pool = &db.pool;
        let owner = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let world = WorldName::parse("party.dcl.eth").unwrap();
        let room = rooms::worlds_service_world_room(&world);
        sqlx::query("INSERT INTO credits.wallets(address,has_started_program) VALUES($1,true)")
            .bind(owner)
            .execute(pool)
            .await
            .unwrap();
        sqlx::raw_sql(
            "INSERT INTO credits.seasons(id,name,start_date,end_date,max_mana_wei)
             VALUES (1,'Test',timezone('UTC',now())-interval '1 day',timezone('UTC',now())+interval '1 day',0);
             INSERT INTO credits.season_goals(season_id,goal_id,title,thumbnail,total_steps,reward_wei,start_week,end_week)
             VALUES (1,'log-in','Log in','login.png',1,0,1,3);
             INSERT INTO social.badge_definitions(id,name,description,category,is_tier,criteria_steps,assets,published_at)
             VALUES ('room-badge','Room badge','Presence','Explorer',false,1,
                     '{\"2d\":{\"normal\":\"room-badge/2d/normal.png\"}}',timezone('UTC',now()));
             INSERT INTO social.presence_badges(badge_id,location_kind,world_name,window_start,window_end,mode,required_seconds)
             VALUES ('room-badge','world','party.dcl.eth',timezone('UTC',now())-interval '1 hour',
                     timezone('UTC',now())+interval '1 hour','visit',0);
             ALTER TABLE credits.goal_room_events ADD CONSTRAINT reject_goal_event CHECK (false);",
        )
        .execute(pool)
        .await
        .unwrap();

        let outbound = crate::http::Outbound::for_tests();
        let credits = Arc::new(shop::credits::State::new(
            pool.clone(),
            outbound.clone(),
            shop::credits::Settings::for_tests(),
            shop::credits::Sources {
                market: None,
                price: None,
                economy: None,
            },
            None,
        ));
        let settings = gatekeeper::Settings::for_tests();
        let body = serde_json::to_vec(&serde_json::json!({
            "id": "EV_retry", "createdAt": chrono::Utc::now().timestamp(), "event": "participant_joined",
            "room": {"name": room}, "participant": {"identity": owner, "sid": "PA_one", "metadata": "{}"}
        })).unwrap();
        let auth = gatekeeper::webhook::test_support::webhook_jwt(
            &body,
            &settings.livekit.api_key,
            &settings.livekit.api_secret,
        );
        let node = gatekeeper::CommsState::new(
            pool.clone(),
            outbound,
            settings,
            None,
            None,
            Arc::new(StubPolicy::default()),
            Arc::new(StubScenes {
                room: Ok(rooms::ResolvedRoom {
                    room,
                    realm: RealmId::World(world),
                    scene_id: None,
                    deployment: None,
                }),
                scene: Ok(None),
            }),
            Arc::new(comms::presence::Presence::new()),
            Arc::new(StubDirectory::default()),
        )
        .with_room_event_store(Arc::new(CreditsRoomEvents {
            credits,
            events_mirror: None,
            badges: Some(Arc::new(social::badges::presence::EventSink::new(
                pool.clone(),
                Default::default(),
            ))),
        }));
        let app = gatekeeper::router(Arc::new(node));
        let post = || {
            app.clone().oneshot(
                Request::post("/livekit-webhook")
                    .header("authorization", &auth)
                    .header("content-type", "application/webhook+json")
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
        };
        assert_eq!(
            post().await.unwrap().status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        let facts = || {
            sqlx::query_as::<_, (i64, i64)>(
                "SELECT (SELECT count(*) FROM social.presence_events),
                        (SELECT count(*) FROM credits.goal_room_events)",
            )
            .fetch_one(pool)
        };
        assert_eq!(facts().await.unwrap(), (1, 0));
        sqlx::query("ALTER TABLE credits.goal_room_events DROP CONSTRAINT reject_goal_event")
            .execute(pool)
            .await
            .unwrap();
        assert_eq!(post().await.unwrap().status(), StatusCode::OK);
        assert_eq!(post().await.unwrap().status(), StatusCode::OK);
        assert_eq!(facts().await.unwrap(), (1, 1));
        db.drop().await;
    }
}

/// Shop's catalogue as the first-wear producer's item source
/// ([`shop::market::items::buyable_item`]): the item as `/v1/items` reads it,
/// buyable when it mints or while `/v1/orders` reads an open listing of it.
/// A refused request (4xx) is final; anything else, an index or mirror not
/// ready included, is retried.
pub(super) struct ShopBuyableItems(pub(super) Arc<shop::market::MarketplaceState>);

#[async_trait::async_trait]
impl social::notifications::first_wear::ItemSource for ShopBuyableItems {
    async fn buyable_item(
        &self,
        urn: &str,
    ) -> Result<
        Option<social::notifications::first_wear::BuyableItem>,
        social::notifications::first_wear::ItemLookupError,
    > {
        use social::notifications::first_wear::{BuyableItem, ItemLookupError};
        match shop::market::items::buyable_item(&self.0, urn).await {
            Ok(item) => Ok(item.map(|item| BuyableItem {
                item_id: item.id,
                name: item.name,
                creator: item.creator.to_lowercase(),
                image: Some(item.thumbnail).filter(|image| !image.is_empty()),
            })),
            Err(response) => {
                let status = response.status();
                let body = axum::body::to_bytes(response.into_body(), 4096)
                    .await
                    .unwrap_or_default();
                let why = format!(
                    "the shop catalogue answered {status}: {}",
                    String::from_utf8_lossy(&body)
                );
                Err(if status.is_client_error() {
                    ItemLookupError::Invalid(why)
                } else {
                    ItemLookupError::Transient(why)
                })
            }
        }
    }
}

#[cfg(test)]
mod shop_buyable_items_tests {
    use super::*;
    use social::notifications::first_wear::{ItemLookupError, ItemSource};

    /// A catalogue that cannot answer yet (nothing published) is a
    /// transient failure the producer retries, carrying the route's status;
    /// never "nothing to buy".
    #[tokio::test]
    async fn an_unready_catalogue_is_retried_not_dropped() {
        use crate::db::scratch::{OwnedDatabase, Schema};
        let Some(db) = OwnedDatabase::new("first_wear_items", Schema::Full, 2).await else {
            return;
        };
        let items = ShopBuyableItems(Arc::new(shop::market::MarketplaceState::new(
            db.pool.clone(),
            crate::http::Outbound::for_tests(),
            shop::market::Settings::for_tests(),
        )));
        let answer = items
            .buyable_item("urn:decentraland:matic:collections-v2:0xeee:1")
            .await;
        assert!(
            matches!(&answer, Err(ItemLookupError::Transient(why)) if why.contains("503")),
            "{answer:?}"
        );
        db.drop().await;
    }
}
