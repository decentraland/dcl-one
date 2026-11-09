//! TLS trust for the helper's http(s) client: the OS root store plus
//! Mozilla's bundle.
//!
//! The OS store is read once, by the helper before it sandboxes itself (the
//! sandbox denies the keychain and the `/etc/ssl` symlink targets); on
//! Windows the helper already runs under its low-integrity token, which may
//! read the certificate stores but not create their registry keys, so they
//! are opened read-only. Verification itself is rustls in-process. The
//! bundle keeps a fresh Windows that has not yet downloaded a root, or a
//! container without `ca-certificates`, working.

use std::collections::HashSet;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use ureq::config::Config;
use ureq::tls::{Certificate, RootCerts, TlsConfig};

pub struct Trust {
    config: Config,
    /// Roots the OS store supplied, duplicates of the bundle included.
    pub system_roots: usize,
    pub bundled_roots: usize,
    /// Store entries that failed to load (unreadable file, corrupt entry,
    /// a store that would not open).
    pub load_errors: Vec<String>,
}

#[derive(Default)]
struct NativeRoots {
    certs: Vec<Vec<u8>>,
    errors: Vec<String>,
}

#[cfg(not(windows))]
fn native_roots() -> NativeRoots {
    let native = rustls_native_certs::load_native_certs();
    NativeRoots {
        certs: native
            .certs
            .iter()
            .map(|cert| cert.as_ref().to_vec())
            .collect(),
        errors: native.errors.iter().map(ToString::to_string).collect(),
    }
}

/// The current user's ROOT logical store, which includes the machine roots,
/// Windows Update's and group policy's; the machine store alone when the
/// user's cannot be opened.
#[cfg(windows)]
fn native_roots() -> NativeRoots {
    let mut options = schannel::cert_store::CertStoreBuilder::new();
    options.read_only();
    let mut roots = NativeRoots::default();
    let store = options.open_current_user("ROOT").or_else(|error| {
        roots
            .errors
            .push(format!("current user ROOT store: {error}"));
        options.open_local_machine("ROOT")
    });
    match store {
        Ok(store) => roots
            .certs
            .extend(store.certs().map(|cert| cert.to_der().to_vec())),
        Err(error) => roots
            .errors
            .push(format!("local machine ROOT store: {error}")),
    }
    roots
}

static TRUST: OnceLock<Trust> = OnceLock::new();

/// Loads the roots on the first call; later calls return the same set.
pub fn init() -> &'static Trust {
    TRUST.get_or_init(load)
}

/// A client over the loaded roots. Each playback owns one, so that
/// cancelling the playback can shut its connections down.
pub(crate) fn agent() -> ureq::Agent {
    ureq::Agent::new_with_config(init().config.clone())
}

fn load() -> Trust {
    let native = native_roots();
    let mut seen = HashSet::new();
    let mut certs = Vec::new();
    let mut add = |der: &[u8]| {
        if seen.insert(der.to_vec()) {
            certs.push(Certificate::from_der(der).to_owned());
        }
    };
    for cert in webpki_root_certs::TLS_SERVER_ROOT_CERTS {
        add(cert.as_ref());
    }
    for cert in &native.certs {
        add(cert);
    }
    let tls = TlsConfig::builder()
        .root_certs(RootCerts::Specific(Arc::new(certs)))
        .build();
    let config = ureq::Agent::config_builder()
        .tls_config(tls)
        .http_status_as_error(false)
        .max_redirects(8)
        .user_agent(concat!("uuav/", env!("CARGO_PKG_VERSION")))
        .timeout_resolve(Some(Duration::from_secs(10)))
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_per_read(Some(Duration::from_secs(30)))
        .build();
    Trust {
        config,
        system_roots: native.certs.len(),
        bundled_roots: webpki_root_certs::TLS_SERVER_ROOT_CERTS.len(),
        load_errors: native.errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_is_present_and_roots_load_once() {
        let first = init();
        assert!(
            first.bundled_roots > 100,
            "Mozilla bundle: {}",
            first.bundled_roots
        );
        assert!(std::ptr::eq(first, init()));
    }
}
