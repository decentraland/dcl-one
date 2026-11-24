//! The route handlers, one file per old handler module; `ping` inlined.

pub mod collections;
pub mod drafts;
pub mod publication;

pub async fn ping() -> &'static str {
    "pong"
}
