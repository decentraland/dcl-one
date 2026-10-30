//! What one request and one file may hold. The bytes come from the page under test, which is not
//! trusted, so each is bounded.

use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// One `csv` or `log` request.
    pub chunk_bytes: u64,
    /// `end` and the reset routes: a few numbers.
    pub control_bytes: u64,
    /// All that one file (a build's CSV, a build's log) holds; past it the server answers 413.
    pub file_bytes: u64,
}

impl Limits {
    pub const DEFAULT: Limits = Limits {
        chunk_bytes: 16 * 1024 * 1024,
        control_bytes: 4 * 1024,
        file_bytes: 256 * 1024 * 1024,
    };
}

/// How long a client may take to send its request headers, and to send the next piece of a body.
/// A request that stalls is dropped instead of holding a stream's lock.
pub const REQUEST_HEADERS: Duration = Duration::from_secs(30);
pub const BODY_IDLE: Duration = Duration::from_secs(30);
