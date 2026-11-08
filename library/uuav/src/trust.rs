//! TLS trust for the helper's http(s) client: the OS root store, with
//! Mozilla's bundle only where that store has no roots.
//!
//! A store that yields roots is the policy on every platform: a root the
//! user or an administrator removed from it is not trusted through the
//! bundle. The bundle stands in when the store yields none (a container
//! without `ca-certificates`). The Windows ROOT store holds only the roots
//! Windows has fetched so far, so a server under a root not fetched yet
//! fails to verify; the certificates in its Disallowed store are dropped
//! from the result.
//!
//! The OS store is read once, by the helper before it sandboxes itself (the
//! sandbox denies the keychain and the `/etc/ssl` symlink targets); on
//! Windows the helper already runs under its low-integrity token, which may
//! read the certificate stores but not create their registry keys, so they
//! are opened read-only. Verification itself is rustls in-process.

use std::collections::HashSet;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use ureq::config::Config;
use ureq::tls::{Certificate, RootCerts, TlsConfig};

pub struct Trust {
    config: Config,
    /// Roots the OS store supplied, duplicates of the bundle included.
    pub system_roots: usize,
    /// Roots of Mozilla's bundle in use: none when the OS store is the policy.
    pub bundled_roots: usize,
    /// Store entries that failed to load (unreadable file, corrupt entry,
    /// a store that would not open).
    pub load_errors: Vec<String>,
}

#[derive(Default)]
struct NativeRoots {
    certs: Vec<Vec<u8>>,
    /// Certificates the OS lists as not trusted (the Windows Disallowed store).
    distrusted: Vec<Vec<u8>>,
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
        distrusted: Vec::new(),
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
    match options.open_current_user("Disallowed") {
        Ok(store) => roots
            .distrusted
            .extend(store.certs().map(|cert| cert.to_der().to_vec())),
        Err(error) => roots
            .errors
            .push(format!("current user Disallowed store: {error}")),
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

/// The roots that verify a server, as DER: the OS store's, or the bundle's
/// when the store yields none, without duplicates and without what the OS
/// distrusts.
fn select<'a>(
    native: &'a [Vec<u8>],
    distrusted: &'a [Vec<u8>],
    bundled: &'a [&'a [u8]],
) -> Vec<&'a [u8]> {
    let bundled = if native.is_empty() { bundled } else { &[] };
    let mut seen: HashSet<&[u8]> = distrusted.iter().map(Vec::as_slice).collect();
    native
        .iter()
        .map(Vec::as_slice)
        .chain(bundled.iter().copied())
        .filter(|der| seen.insert(der))
        .collect()
}

fn load() -> Trust {
    let native = native_roots();
    let bundled: Vec<&[u8]> = webpki_root_certs::TLS_SERVER_ROOT_CERTS
        .iter()
        .map(AsRef::as_ref)
        .collect();
    let roots = select(&native.certs, &native.distrusted, &bundled);
    let bundled_roots = roots
        .iter()
        .filter(|der| !native.certs.iter().any(|cert| cert == *der))
        .count();
    let certs: Vec<_> = roots
        .iter()
        .map(|der| Certificate::from_der(der).to_owned())
        .collect();
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
        bundled_roots,
        load_errors: native.errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLED: &[&[u8]] = &[b"shared", b"bundle only"];

    fn native(certs: &[&[u8]]) -> Vec<Vec<u8>> {
        certs.iter().map(|cert| cert.to_vec()).collect()
    }

    #[test]
    fn a_root_the_os_store_lacks_is_not_trusted_through_the_bundle() {
        let os = native(&[b"shared", b"corporate"]);
        assert_eq!(
            select(&os, &[], BUNDLED),
            [b"shared".as_slice(), b"corporate"]
        );
    }

    #[test]
    fn the_bundle_stands_in_for_a_store_without_roots() {
        assert_eq!(
            select(&[], &[], BUNDLED),
            [b"shared".as_slice(), b"bundle only"]
        );
    }

    #[test]
    fn what_the_os_distrusts_is_dropped_from_the_store_and_the_bundle() {
        let os = native(&[b"shared", b"corporate"]);
        let distrusted = native(&[b"bundle only", b"corporate"]);
        assert_eq!(select(&os, &distrusted, BUNDLED), [b"shared".as_slice()]);
        assert_eq!(select(&[], &distrusted, BUNDLED), [b"shared".as_slice()]);
    }

    #[test]
    fn roots_load_once_and_the_bundle_is_present() {
        let first = init();
        assert!(webpki_root_certs::TLS_SERVER_ROOT_CERTS.len() > 100);
        assert!(first.system_roots > 0 || first.bundled_roots > 100);
        assert!(std::ptr::eq(first, init()));
    }
}
