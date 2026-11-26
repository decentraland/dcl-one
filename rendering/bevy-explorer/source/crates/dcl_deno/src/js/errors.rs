use std::borrow::Cow;

use deno_core::{anyhow, error::ResourceError};
use deno_error::{AdditionalProperties, JsErrorBox, JsErrorClass};

/// The error type of every op: ops raise `anyhow` errors, and op2 needs a `JsErrorClass`.
/// A `JsErrorBox` inside the `anyhow` chain keeps its class; anything else is an `Error`
/// carrying the whole context chain as its message.
#[derive(Debug)]
pub struct OpError(JsErrorBox);

impl std::fmt::Display for OpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::error::Error for OpError {}

impl From<anyhow::Error> for OpError {
    fn from(error: anyhow::Error) -> Self {
        let error = match error.downcast::<JsErrorBox>() {
            Ok(js) => return Self(js),
            Err(error) => error,
        };
        match error.downcast::<ResourceError>() {
            Ok(resource) => Self(JsErrorBox::from_err(resource)),
            Err(error) => Self(JsErrorBox::generic(format!("{error:#}"))),
        }
    }
}

/// Errors ops raise with `?` directly.
macro_rules! from_std_error {
    ($($error:ty),* $(,)?) => {
        $(impl From<$error> for OpError {
            fn from(error: $error) -> Self {
                anyhow::Error::from(error).into()
            }
        })*
    };
}

from_std_error!(
    ResourceError,
    http::method::InvalidMethod,
    reqwest::Error,
    tokio::runtime::TryCurrentError,
);

impl From<JsErrorBox> for OpError {
    fn from(error: JsErrorBox) -> Self {
        Self(error)
    }
}

impl JsErrorClass for OpError {
    fn get_class(&self) -> Cow<'static, str> {
        self.0.get_class()
    }

    fn get_message(&self) -> Cow<'static, str> {
        self.0.get_message()
    }

    fn get_additional_properties(&self) -> AdditionalProperties {
        self.0.get_additional_properties()
    }

    fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
        self
    }
}

pub fn type_error(message: impl Into<Cow<'static, str>>) -> anyhow::Error {
    JsErrorBox::type_error(message).into()
}

pub fn generic_error(message: impl Into<Cow<'static, str>>) -> anyhow::Error {
    JsErrorBox::generic(message).into()
}

/// `anyhow::bail!` for functions returning either `AnyError` or `OpError`.
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err(deno_core::anyhow::anyhow!($($arg)*).into())
    };
}
pub(crate) use bail;
