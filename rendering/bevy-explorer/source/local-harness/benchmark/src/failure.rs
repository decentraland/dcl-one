//! What stops a command, and the exit code each kind ends with.

use std::fmt;
use std::io;

use crate::out;

/// A mistyped option, or an input the runner refuses.
pub const EXIT_USAGE: i32 = 64;
/// The machine cannot run the leg, or the disk or the network failed.
pub const EXIT_CANNOT_RUN: i32 = 70;
pub const EXIT_INTERRUPTED: i32 = 130;

#[derive(Debug)]
pub enum Failure {
    /// An option or input that is wrong; the message says how.
    Usage(String),
    /// The machine cannot run the leg at all (no Chrome, port taken, no network), as opposed
    /// to a run that failed.
    CannotRun(String),
    /// What the disk or the network did, as a one-line message.
    Failed(String),
    Interrupted,
}

impl Failure {
    pub fn usage(message: impl Into<String>) -> Self {
        Self::Usage(message.into())
    }

    pub fn cannot_run(message: impl Into<String>) -> Self {
        Self::CannotRun(message.into())
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => EXIT_USAGE,
            Self::CannotRun(_) | Self::Failed(_) => EXIT_CANNOT_RUN,
            Self::Interrupted => EXIT_INTERRUPTED,
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) => write!(f, "usage: {message}"),
            Self::CannotRun(message) => write!(f, "cannot run: {message}"),
            Self::Failed(message) => write!(f, "failed: {message}"),
            Self::Interrupted => write!(f, "interrupted"),
        }
    }
}

impl std::error::Error for Failure {}

// A message, not a stack trace, for what the disk or the network does.
impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        Self::Failed(out::safe(&error.to_string()))
    }
}

// Without the address the error was for: its query may be a signature (see `app_source`).
impl From<reqwest::Error> for Failure {
    fn from(error: reqwest::Error) -> Self {
        Self::Failed(out::safe(&error.without_url().to_string()))
    }
}

impl From<zip06::result::ZipError> for Failure {
    fn from(error: zip06::result::ZipError) -> Self {
        Self::Usage(format!(
            "the archive cannot be read as a zip ({})",
            out::safe(&error.to_string())
        ))
    }
}
