//! The framing messages of `index.proto` (`RpcMessageHeader`, `CreatePort`,
//! `Request`, `StreamMessage`, ...). `generated.rs` is the prost 0.14.4
//! output for `index.proto`, regenerated with:
//!
//! ```sh
//! protox index.proto | prost-build   # a scratch crate: protox 0.9.1 + prost-build 0.14.4,
//!                                    # Config::new().compile_fds(fds) -> _.rs
//! ```
//!
//! The `.proto` beside it is the source of truth; the two are committed
//! together so the wire contract is reviewable without a build-time protoc.

pub mod parse;

include!("generated.rs");

/// Implemented by the error type a server procedure returns; the code and
/// message travel to the client as a [`RemoteError`].
pub trait RemoteErrorResponse {
    fn error_code(&self) -> u32;
    fn error_message(&self) -> String;
}

impl<T: RemoteErrorResponse> From<T> for RemoteError {
    fn from(value: T) -> Self {
        Self {
            message_identifier: 0,
            error_code: value.error_code(),
            error_message: value.error_message(),
        }
    }
}

/// The `From<T>` conversion above cannot know the `message_number`, so it leaves
/// `message_identifier` at `0` and this fills it in once the number is known.
pub(crate) fn fill_remote_error(remote_error: &mut RemoteError, message_number: u32) {
    remote_error.message_identifier = parse::build_message_identifier(
        RpcMessageTypes::RemoteErrorResponse as u32,
        message_number,
    );
}

pub(crate) fn server_ready_message() -> RpcMessageHeader {
    RpcMessageHeader {
        message_identifier: parse::build_message_identifier(RpcMessageTypes::ServerReady as u32, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    enum MyEnumOfErrors {
        EntityNotFound,
    }

    impl RemoteErrorResponse for MyEnumOfErrors {
        fn error_code(&self) -> u32 {
            404
        }
        fn error_message(&self) -> String {
            "The entity wasn't found".to_string()
        }
    }

    #[test]
    fn a_procedure_error_becomes_a_remote_error() {
        let error: RemoteError = MyEnumOfErrors::EntityNotFound.into();
        assert_eq!(error.error_code, 404);
        assert_eq!(error.error_message, "The entity wasn't found");
        assert_eq!(error.message_identifier, 0);
    }
}
