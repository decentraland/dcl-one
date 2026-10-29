//! Moderation: bans, warnings, scene admins. The routes
//! (`handlers`) over the moderation relations of `social`
//! (`store`).

pub mod handlers;
pub mod maintenance;
pub mod store;

mod ban;
mod context;
#[cfg(any(test, feature = "test-support"))]
pub use ban::test_support;
pub use ban::{BanStatus, admission_ban};
pub use store::Moderation;
