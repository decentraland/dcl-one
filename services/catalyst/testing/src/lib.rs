//! Test support the unit tests and the integration test binaries share.
//! Compiled into the library because an integration test links the
//! non-`cfg(test)` build and cannot see a `cfg(test)` module.

// A silently truncated, wrapped or rounded cast is a bug.
#![deny(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]

pub mod conformance;
