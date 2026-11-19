//! The Cast presenter: an in-world presentation, PDF slides published into
//! the scene's LiveKit room as a video track and driven over the data
//! channel, in place of upstream's Node service
//! (`decentraland/cast-presenter-server` @ cae78612).
//!
//! `serve` mounts [`router`] under `/cast-presenter-service`, upstream's host
//! label, so `POST <node>/cast-presenter-service/presentations` is the route
//! (and the gateway's subdomain rule sends
//! `cast-presenter-service.<domain>/presentations` there). The Admin Toolkit
//! gets the bot's token from `POST /comms-gatekeeper/cast/presentation-bot-token`
//! and posts it here with the file or its URL.
//!
//! What this node does:
//! - `POST /presentations` with upstream's request forms (multipart upload or
//!   JSON `url`), checks (extension, magic bytes, 100 MiB file, 1 MiB JSON
//!   body, the download's SSRF guard), `MAX_CONCURRENT_PRESENTATIONS` (429)
//!   and answers ([`http`]).
//! - `livekitUrl` must be this node's LiveKit, the URL the gatekeeper's
//!   presentation-bot token answer names (400 otherwise); the bot dials the
//!   node's own route to it, never the request's URL ([`session::LiveKitTarget`]).
//! - The token must be a presentation-bot token of this node's LiveKit (its
//!   key's signature, unexpired, a `presentation-bot:` identity of the room
//!   it joins, allowed to publish a screen share): any other answers 401
//!   before a slot is taken or a bot started ([`session`]). Multipart uploads
//!   are read under a permit, as many at once as there are slots (429).
//! - The bot then connects with the token before any rendering; a token
//!   LiveKit refuses answers 401 on the JSON path (checked before the
//!   download; 503 when the bot or LiveKit is unavailable) and 500 on the
//!   multipart path, as upstream.
//! - PDF slides ([`render`] names the renderer and why), one SCREENSHARE
//!   track, the commands `navigate`, `get-state`, `stop` and
//!   `overlay:update` from the room's presenters, the state broadcast and
//!   bot metadata ([`wire`]), and the idle reaper ([`session`]).
//!
//! Not yet: PPTX (answers 422), embedded video playback (`video:play`
//! answers upstream's `presentation:error` event, the other `video:*`
//! commands are ignored and `videoState` stays `idle`), client composition (no
//! `slide` field, no `GET /presentations/:id/slides/:file`,
//! `presenter:claim` ignored) and upstream's `/api/drive-video` and `/ping`.
//!
//! The LiveKit connection lives in a `cast-bot` process ([`bot`]); with no
//! `CAST_BOT_BIN` or no LiveKit configured the route answers 503.

pub mod bot;
pub mod files;
pub mod http;
pub mod render;
pub mod session;
pub mod ssrf;
pub mod wire;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use axum::Router;

use crate::shared::routing::post;

pub use session::{LiveKitTarget, Presenter};

/// A presenter that starts `binary` for each session against the node's
/// LiveKit, or none without both.
pub fn presenter(
    binary: Option<std::path::PathBuf>,
    livekit: Option<LiveKitTarget>,
    max_concurrent: usize,
) -> Option<Arc<Presenter>> {
    let presenter = Presenter::new(
        Arc::new(bot::ProcessLauncher { binary: binary? }),
        livekit?,
        max_concurrent,
    );
    presenter.spawn_reaper();
    Some(presenter)
}

/// `POST /presentations`, with upstream's security headers and its CORS
/// (any origin, no credentials: the request is authorized by the LiveKit
/// token in its body): the Cast page posts the JSON form cross-origin, which
/// a browser sends only after a preflight. This node also sends a max-age.
const CORS: crate::shared::cors::HttpServerCors = crate::shared::cors::HttpServerCors {
    methods: Some("GET,HEAD,PUT,PATCH,POST,DELETE"),
    max_age: "86400",
    nosniff: false,
    preflight_echoes_request: false,
};

pub fn router(presenter: Option<Arc<Presenter>>) -> Router {
    Router::new()
        .route("/presentations", post(http::create_presentation))
        .layer(axum::middleware::from_fn(http::security_headers))
        .layer(axum::middleware::from_fn(|request, next| {
            CORS.run(request, next)
        }))
        .with_state(presenter)
}
