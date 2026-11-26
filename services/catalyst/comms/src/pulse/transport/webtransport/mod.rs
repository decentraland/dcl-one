#[cfg(feature = "test-support")]
pub mod client;
pub mod config;
pub mod framing;
pub mod host;
pub mod quic;

pub use config::WtConfig;
pub use host::{WtHost, WtMetrics};

/// The TLS provider for every WebTransport endpoint: aws-lc-rs, named here so the transport's TLS does
/// not follow whichever provider a process happens to install, or `web-transport-quinn`'s own default.
pub(crate) fn tls_provider() -> std::sync::Arc<rustls::crypto::CryptoProvider> {
    std::sync::Arc::new(rustls::crypto::aws_lc_rs::default_provider())
}

pub const CHANNEL_SEQUENCED: u8 = 1;
