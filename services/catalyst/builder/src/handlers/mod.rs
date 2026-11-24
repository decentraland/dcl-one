//! The route handlers, one file per old handler module; `ping` inlined.

pub mod collections;
pub mod curation;
pub mod drafts;
pub mod forum;
pub mod lifecycle;
pub mod newsletter;
pub mod publication;
pub mod review;
pub mod small_reads;
pub mod storage;
pub mod third_parties;
pub mod validator;

pub async fn ping() -> &'static str {
    "pong"
}
