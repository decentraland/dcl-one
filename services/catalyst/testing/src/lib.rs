//! Test support the unit tests and the integration test binaries share.
//! Compiled into the library because an integration test links the
//! non-`cfg(test)` build and cannot see a `cfg(test)` module.

pub mod conformance;
