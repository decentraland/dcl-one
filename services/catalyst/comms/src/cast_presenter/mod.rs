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
//! - A room holds several presentations, as upstream, each with a bot token
//!   of its own: a token whose bot is running answers 409 (upstream starts
//!   it, and LiveKit drops the first bot). A command names no presentation,
//!   so every bot of the room obeys a presenter. A client shows one of them:
//!   the Cast pages and the Bevy Explorer the first bot they saw, the Unity
//!   Explorer the first in its participant table, so two viewers can see
//!   different slides.
//! - A bot whose process ended without leaving is removed from its room at
//!   once, after the node sends `presentation:stopped` in its place.
//!   Upstream leaves it to LiveKit, which keeps it about 20 s.
//! - The bot then connects with the token before any rendering; a token
//!   LiveKit refuses answers 401 on the JSON path (checked before the
//!   download; 503 when the bot or LiveKit is unavailable) and 500 on the
//!   multipart path, as upstream.
//! - PDF slides ([`render`] names the renderer and why), one SCREENSHARE
//!   track, the commands `navigate`, `get-state`, `stop` and
//!   `overlay:update` from the room's presenters, the state broadcast and
//!   bot metadata ([`wire`]), and the idle reaper ([`session`]).
//! - Client composition, with `CAST_PRESENTER_PUBLIC_URL` only: the state
//!   names the current slide as a picture (`slide`) and the presenter
//!   (`presenterIdentity`: the request's, then a presenter's
//!   `presentation:presenter:claim`), and `GET /presentations/{id}/slides/{hash}.png`
//!   serves a session's last 8 slides. The Unity Explorer composes from
//!   these. The SCREENSHARE track stays for the clients that show it;
//!   upstream drops it
//!   (`docs/adr/2026-11-20-cast-client-composition.md`).
//!
//! Not yet: PPTX (answers 422), embedded video playback (`video:play`
//! answers upstream's `presentation:error` event, the other `video:*`
//! commands are ignored, `videoState` stays `idle`, and there is no
//! `presentation-video` track or `slideVideos[i].source`) and upstream's
//! `/api/drive-video` and `/ping`.
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

use crate::shared::routing::{get, post};

pub use session::{LiveKitTarget, Presenter};

/// A presenter that starts `binary` for each session against the node's
/// LiveKit, or none without both. `rooms` removes a bot that ended without
/// leaving.
pub fn presenter(
    rooms: Arc<dyn bot::Rooms>,
    binary: Option<std::path::PathBuf>,
    livekit: Option<LiveKitTarget>,
    max_concurrent: usize,
    public_url: Option<String>,
) -> Option<Arc<Presenter>> {
    let presenter = Presenter::new(
        Arc::new(bot::ProcessLauncher { binary: binary? }),
        livekit?,
        max_concurrent,
        public_url,
    );
    presenter.spawn_reaper();
    presenter.set_room_service(rooms);
    Some(presenter)
}

/// The routes, with upstream's security headers and its CORS
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
        .route(
            "/presentations/{id}/slides/{file}",
            get(http::slide_picture),
        )
        .layer(axum::middleware::from_fn(http::security_headers))
        .layer(axum::middleware::from_fn(|request, next| {
            CORS.run(request, next)
        }))
        .with_state(presenter)
}
