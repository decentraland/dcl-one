//! The route handlers, one file per old handler module; `ping` inlined.

pub mod catalog;
pub mod collections;
pub mod curation;
pub mod drafts;
pub mod linked_publication;
pub mod newsletter;
pub mod onchain;
pub mod projects;
pub mod publication;
pub mod storage;

pub async fn ping() -> &'static str {
    "pong"
}
