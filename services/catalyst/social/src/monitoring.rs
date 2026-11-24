use axum::{Json, Router};
use serde_json::json;

use crate::profile_images;
use crate::shared::{health, routing::get};

pub(super) fn routes(
    version: Option<&str>,
    commit: Option<&str>,
    profiles: profile_images::SharedState,
) -> Router {
    let checks = health::status_checks_router_with(health::SERVER_PASS);
    let (processor_version, processor_commit) = (
        version.unwrap_or_default().to_owned(),
        commit.unwrap_or_default().to_owned(),
    );
    let processor = checks.clone().route(
        "/status",
        get(move || async move {
            Json(json!({
                "version": processor_version,
                "commitHash": processor_commit,
                "currentTime": chrono::Utc::now().timestamp_millis(),
            }))
        }),
    );
    let (profile_version, profile_commit) = (
        version
            .filter(|s| !s.is_empty())
            .unwrap_or("Unknown")
            .to_owned(),
        commit
            .filter(|s| !s.is_empty())
            .unwrap_or("Unknown")
            .to_owned(),
    );
    let images = checks.clone().route(
        "/status",
        get(move || async move {
            let (waiting, active) = profiles.producer.queue_counts();
            let queue = |waiting: usize, active: usize| {
                json!({
                    "ApproximateNumberOfMessages": waiting.to_string(),
                    "ApproximateNumberOfMessagesNotVisible": active.to_string(),
                    "ApproximateNumberOfMessagesDelayed": "0",
                })
            };
            Json(json!({
                "version": profile_version,
                "commitHash": profile_commit,
                "queues": { "queue": queue(waiting, active), "dlQueue": queue(0, 0) },
            }))
        }),
    );
    Router::new()
        .nest("/badges-processor", processor)
        .nest("/profile-images", images)
        .nest("/badges", checks.clone())
        .nest("/notifications", checks.clone())
        .nest("/events-notifier", checks)
}
