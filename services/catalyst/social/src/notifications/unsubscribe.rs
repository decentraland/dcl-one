// from: notifications-workers@e2af2ee inbox/src/controllers/handlers/unsubscription-handlers.ts, inbox/src/controllers/routes.ts (signedUrlMiddleware), inbox/src/adapters/page-templates (Apache-2.0)
//! The one-click unsubscribe links of notification emails. The processor
//! signed `new URL('/unsubscribe/…', SERVICE_BASE_URL)` and the inbox
//! verified the request URL, so a link verifies here as `service_base_url`'s
//! origin plus the path below the `/notifications` mount and its query.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::Uri;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use serde_json::{Value as Json, json};

use super::email::html_escape;
use super::errors::{ApiError, NotificationsError};
use super::signing::has_valid_signature;
use super::types::{is_known_type, normalize_details};
use crate::identity::Address;

type AppState = Arc<super::State>;

const ALL_PAGE: &str = include_str!("pages/unsubscription-all.html");
const ONE_PAGE: &str = include_str!("pages/unsubscription-one.html");

/// The URL the email carried, rebuilt from the configured public origin.
pub fn public_url(service_base_url: &str, local: &Uri) -> Option<String> {
    let path = local.path_and_query()?.as_str();
    Some(
        url::Url::parse(service_base_url)
            .ok()?
            .join(path)
            .ok()?
            .into(),
    )
}

fn verify(state: &AppState, local: &Uri) -> Result<(), NotificationsError> {
    let valid = match (
        &state.settings.signing_key,
        public_url(&state.settings.service_base_url, local),
    ) {
        (Some(key), Some(url)) if !key.is_empty() => has_valid_signature(key, &url),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(ApiError::unauthorized("Invalid URL.").into())
    }
}

fn parse_address(raw: &str) -> Result<Address, NotificationsError> {
    Address::parse(raw).map_err(|_| ApiError::bad_request("Invalid address").into())
}

/// Reads the full details, applies `edit`, stores them back (the old handlers' round trip).
async fn update_details(
    state: &AppState,
    address: &Address,
    edit: impl FnOnce(&mut Json),
) -> Result<(), NotificationsError> {
    let current = state.store.get_subscription(address).await?;
    let mut details = current.map_or_else(|| normalize_details(&json!({})), |s| s.details);
    edit(&mut details);
    state
        .store
        .put_subscription_details(address, &details)
        .await?;
    Ok(())
}

fn html(body: String) -> Response {
    ([(CONTENT_TYPE, "text/html")], body).into_response()
}

pub async fn unsubscribe_all(
    State(state): State<AppState>,
    Path(address): Path<String>,
    local: Uri,
) -> Result<Response, NotificationsError> {
    verify(&state, &local)?;
    let address = parse_address(&address)?;
    update_details(&state, &address, |d| d["ignore_all_email"] = json!(true)).await?;
    let user_name = crate::rpc::profiles::Profiles::new(Some(state.store.pool().clone()))
        .get_profile(address.as_str())
        .await
        .map(|p| p.name)
        .filter(|n| !n.is_empty());
    state.track(
        "user_unsubscribed",
        json!({ "address": address.as_str(), "notification_type": "all" }),
    );
    let heading = user_name.map_or_else(String::new, |name| {
        format!(
            "<h1 style=\"margin: 20px 0 10px; color: #000;\">Hey there <spanstyle=\"color: #FF2D55;\">{}</span>!</h1>",
            html_escape(&name)
        )
    });
    Ok(html(ALL_PAGE.replace("{{heading}}", &heading).replace(
        "{{accountLink}}",
        &html_escape(&state.settings.email.account_base_url),
    )))
}

pub async fn unsubscribe_one(
    State(state): State<AppState>,
    Path((address, kind)): Path<(String, String)>,
    local: Uri,
) -> Result<Response, NotificationsError> {
    verify(&state, &local)?;
    let address = parse_address(&address)?;
    if !is_known_type(&kind) {
        return Err(ApiError::bad_request(format!("Invalid notification type: {kind}")).into());
    }
    update_details(&state, &address, |d| {
        d["message_type"][kind.as_str()]["email"] = json!(false)
    })
    .await?;
    state.track(
        "user_unsubscribed",
        json!({ "address": address.as_str(), "notification_type": kind }),
    );
    Ok(html(
        ONE_PAGE
            .replace("{{ notificationType }}", &html_escape(&kind))
            .replace(
                "{{accountLink}}",
                &html_escape(&state.settings.email.account_base_url),
            ),
    ))
}
