//! The plane switch a handler's side write reads: the [`Switch`] a plane's
//! gate inserts on every request it passes, and [`side_refusal`], which a
//! `GET` that writes as a side effect asks. The gates and the refusal itself
//! stay in `shared::writes`.

use axum::extract::Extension;

use crate::shared::writes::{Refusal, refusal};

/// The switch as a plane's gate saw it, on every request it passes: a `GET`
/// that writes as a side effect (the MLS key-package claim, the referral
/// viewed mark, the DM token's connection record) asks [`side_refusal`].
/// Social's `guard` inserts it.
#[derive(Clone, Copy, Debug)]
pub struct Switch(pub bool);

/// `None` lets a handler's side write through; a request that did not pass
/// the gate counts as off.
pub fn side_refusal(switch: Option<Extension<Switch>>) -> Option<Refusal> {
    refusal(switch.is_some_and(|Extension(Switch(on))| on))
}
