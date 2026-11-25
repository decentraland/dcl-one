use serde_json::{Value, json};

/// Why a scoped login may not perform the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopedErrorCode {
    OperationRequired,
    ResourceRequired,
    OperationDenied,
    ResolutionFailed,
}

impl ScopedErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OperationRequired => "SCOPED_OPERATION_REQUIRED",
            Self::ResourceRequired => "SCOPED_RESOURCE_REQUIRED",
            Self::OperationDenied => "SCOPED_OPERATION_DENIED",
            Self::ResolutionFailed => "SCOPED_RESOLUTION_FAILED",
        }
    }
}

/// A refused signed request, with the HTTP status and code it answers.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RequestAuthError {
    /// 400: a header that is not what the protocol carries.
    #[error("{0}")]
    Malformed(String),
    /// 401: the signature does not hold, or the timestamp is outside the window.
    #[error("{0}")]
    Unauthorized(String),
    /// 403: a scoped login that does not grant this request.
    #[error("{message}")]
    Forbidden {
        code: ScopedErrorCode,
        message: String,
    },
    /// 503: a contract wallet's signature could not be checked.
    #[error("Signature verification unavailable")]
    Unavailable,
    /// 429: the client source spent its budget of contract-wallet checks.
    #[error("Too many contract-wallet signature checks; retry later")]
    RateLimited,
}

impl RequestAuthError {
    pub fn status(&self) -> u16 {
        match self {
            Self::Malformed(_) => 400,
            Self::Unauthorized(_) => 401,
            Self::Forbidden { .. } => 403,
            Self::Unavailable => 503,
            Self::RateLimited => 429,
        }
    }

    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::Forbidden { code, .. } => Some(code.as_str()),
            _ => None,
        }
    }

    /// The response body: `{ ok: false, message, code? }`, with a 5xx message withheld.
    pub fn body(&self) -> Value {
        if self.status() >= 500 {
            return json!({ "ok": false, "message": "Internal error" });
        }
        let mut body = json!({ "ok": false, "message": self.to_string() });
        if let Some(code) = self.code() {
            body["code"] = Value::String(code.into());
        }
        body
    }
}

/// A received value echoed in a message, cut to 64 characters.
pub(crate) fn safe(value: &str) -> String {
    let mut chars = value.chars();
    let head: String = chars.by_ref().take(64).collect();
    if chars.next().is_some() {
        format!("{head}\u{2026}")
    } else {
        head
    }
}

pub(crate) fn malformed(message: impl Into<String>) -> RequestAuthError {
    RequestAuthError::Malformed(message.into())
}

pub(crate) fn unauthorized(message: impl Into<String>) -> RequestAuthError {
    RequestAuthError::Unauthorized(message.into())
}

pub(crate) fn forbidden(code: ScopedErrorCode, message: impl Into<String>) -> RequestAuthError {
    RequestAuthError::Forbidden {
        code,
        message: message.into(),
    }
}
