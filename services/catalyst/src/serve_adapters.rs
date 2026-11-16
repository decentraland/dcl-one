//! What `serve` hands Shop from Social, and Shop's answer Comms asks for: Shop
//! builds without either crate, so these wrappers connect them here.

use std::sync::Arc;

use crate::shared::identity::Address;
use crate::shared::notifications::SendFailure;
use crate::{comms, shop, social};

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
