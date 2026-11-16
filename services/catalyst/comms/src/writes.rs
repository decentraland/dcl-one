//! The plane switch a handler's side write reads: the [`Switch`] a plane's
//! gate inserts on every request it passes, and [`side_writes_on`], which a
//! `GET` that writes as a side effect asks. The gates themselves stay in
//! `shared::writes`.

use axum::extract::Extension;

/// The switch as a plane's gate saw it, on every request it passes: a `GET`
/// that writes as a side effect (the MLS key-package claim, the referral
/// viewed mark, the DM token's connection record) asks [`side_writes_on`].
/// Social's `guard` inserts it.
#[derive(Clone, Copy, Debug)]
pub struct Switch(pub bool);

/// Whether a handler's side write may go through; a request that did not
/// pass the gate counts as off.
pub fn side_writes_on(switch: Option<Extension<Switch>>) -> bool {
    switch.is_some_and(|Extension(Switch(on))| on)
}
