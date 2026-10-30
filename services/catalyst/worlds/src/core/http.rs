//! The worlds content server's error envelope (`ApiError`, `envelope`,
//! `clamp_limit`) under the path the worlds routes import it from; it lives
//! in `comms::worlds_envelope`, shared with the world comms mints in comms.
//! The upload-limit answers are re-exported by `upload_limits`.

pub use crate::comms::worlds_envelope::{ApiError, clamp_limit, envelope};
