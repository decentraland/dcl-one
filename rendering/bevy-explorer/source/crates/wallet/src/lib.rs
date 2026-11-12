use std::sync::{Arc, RwLock};

use alloy_core::primitives::Address;
use alloy_signer::{Error as WalletError, Signature, Signer};
use alloy_signer_local::PrivateKeySigner;
use anyhow::anyhow;
use async_trait::async_trait;
use bevy::prelude::*;
use common::structs::ChainLink;
use http::Uri;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

pub mod browser_auth;
pub mod delegation;
#[cfg(not(target_arch = "wasm32"))]
pub mod local_guest;
pub mod remote_signer;
pub mod signed_login;

pub struct WalletPlugin;

impl Plugin for WalletPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Wallet>();
    }
}

#[derive(Resource, Clone, Default)]
pub struct Wallet(Arc<RwLock<WalletInner>>);

#[derive(Default)]
struct WalletInner {
    pub(crate) inner: Option<Arc<dyn ObjSafeWalletSigner + 'static + Send + Sync>>,
    pub(crate) root_address: Option<Address>,
    pub(crate) delegates: Vec<ChainLink>,
    pub(crate) guest_account: bool,
}

impl Wallet {
    pub fn auth_chain(&self) -> Result<SimpleAuthChain, anyhow::Error> {
        let inner = self.0.read().unwrap();

        Ok(SimpleAuthChain::new_chain(
            *inner
                .root_address
                .as_ref()
                .ok_or(anyhow!("wallet not connected"))?,
            &inner.delegates,
        ))
    }

    /// Freeze the signer and auth chain for an in-flight request across account changes.
    pub fn snapshot(&self) -> Self {
        let read = self.0.read().unwrap();
        Self(Arc::new(RwLock::new(WalletInner {
            inner: read.inner.clone(),
            root_address: read.root_address,
            delegates: read.delegates.clone(),
            guest_account: read.guest_account,
        })))
    }

    pub fn disconnect(&mut self) {
        let mut write = self.0.write().unwrap();
        write.inner = None;
        write.root_address = None;
        write.delegates.clear();
        write.guest_account = false;
    }

    pub fn finalize_as_guest(&mut self) {
        let inner: Arc<dyn ObjSafeWalletSigner + Send + Sync> =
            Arc::new(PrivateKeySigner::random_with(&mut rand::thread_rng()));
        let mut write = self.0.write().unwrap();
        write.root_address = Some(inner.address());
        write.delegates.clear();
        write.guest_account = false;
        write.inner = Some(inner);
    }

    pub fn finalize_as_guest_with_seed(&mut self, seed: [u8; 32]) {
        let inner: Arc<dyn ObjSafeWalletSigner + Send + Sync> = Arc::new(
            PrivateKeySigner::random_with(&mut rand::rngs::StdRng::from_seed(seed)),
        );
        let mut write = self.0.write().unwrap();
        write.root_address = Some(inner.address());
        write.delegates.clear();
        write.guest_account = false;
        write.inner = Some(inner);
    }

    pub fn finalize(
        &mut self,
        root_address: Address,
        local_wallet: PrivateKeySigner,
        auth: Vec<ChainLink>,
        guest_account: bool,
    ) {
        let mut write = self.0.write().unwrap();
        write.root_address = Some(root_address);
        write.delegates = auth;
        write.guest_account = guest_account;
        write.inner = Some(Arc::new(local_wallet));
    }

    /// Like `finalize`, for an ephemeral key held outside the engine (`remote_signer`).
    pub fn finalize_remote(
        &mut self,
        root_address: Address,
        ephemeral_address: Address,
        auth: Vec<ChainLink>,
        guest_account: bool,
    ) {
        let mut write = self.0.write().unwrap();
        write.root_address = Some(root_address);
        write.delegates = auth;
        write.guest_account = guest_account;
        write.inner = Some(Arc::new(remote_signer::RemoteSigner {
            address: ephemeral_address,
        }));
    }

    pub async fn sign_message(&self, message: String) -> Result<SimpleAuthChain, WalletError> {
        // not held across the signing: a remote signer awaits the page, and logging in or out
        // meanwhile needs the write lock
        let (signer, root_address, delegates) = {
            let read = self.0.read().unwrap();
            let signer = read.inner.clone().ok_or_else(|| {
                WalletError::Other(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "wallet not connected",
                )))
            })?;
            (signer, read.root_address.unwrap(), read.delegates.clone())
        };
        signer.sign_message(message, root_address, &delegates).await
    }

    pub fn address(&self) -> Option<Address> {
        self.0.read().unwrap().root_address
    }

    pub fn is_guest(&self) -> bool {
        self.0.read().unwrap().delegates.is_empty()
    }

    /// An account that was created for a guest. It signs in and keeps a profile like any
    /// other account (so `is_guest` is false), but the user holds no wallet to approve
    /// transactions with.
    pub fn is_guest_account(&self) -> bool {
        self.0.read().unwrap().guest_account
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub(crate) trait ObjSafeWalletSigner {
    async fn sign_message(
        &self,
        message: String,
        root_address: Address,
        delegates: &[ChainLink],
    ) -> Result<SimpleAuthChain, WalletError>;

    // /// Signs the transaction
    // async fn sign_transaction(&self, message: &TypedTransaction) -> Result<Signature, WalletError>;

    /// Returns the signer's Ethereum Address
    fn address(&self) -> Address;

    // /// Returns the signer's chain id
    // fn chain_id(&self) -> u64;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl ObjSafeWalletSigner for PrivateKeySigner {
    async fn sign_message(
        &self,
        message: String,
        root_address: Address,
        delegates: &[ChainLink],
    ) -> Result<SimpleAuthChain, WalletError> {
        let signature = Signer::sign_message(self, message.as_bytes()).await?;
        Ok(SimpleAuthChain::new(
            root_address,
            delegates,
            message,
            signature,
        ))
    }

    // async fn sign_transaction(&self, message: &TypedTransaction) -> Result<Signature, WalletError> {
    //     Signer::sign_transaction(self, message).await
    // }

    fn address(&self) -> Address {
        Signer::address(self)
    }

    // fn chain_id(&self) -> u64 {
    //     Signer::chain_id(self)
    // }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SimpleAuthChain(Vec<ChainLink>);

impl SimpleAuthChain {
    pub fn new(
        signer_address: Address,
        delegates: &[ChainLink],
        payload: String,
        signature: Signature,
    ) -> Self {
        let mut links = Vec::with_capacity(delegates.len() + 2);
        links.push(ChainLink {
            ty: "SIGNER".to_owned(),
            payload: format!("{signer_address:#x}"),
            signature: String::default(),
        });
        links.extend(delegates.iter().cloned());
        links.push(ChainLink {
            ty: "ECDSA_SIGNED_ENTITY".to_owned(),
            payload,
            signature: signature.to_string(),
        });
        Self(links)
    }

    pub fn new_chain(signer_address: Address, delegates: &[ChainLink]) -> Self {
        let mut links = Vec::with_capacity(delegates.len() + 1);
        links.push(ChainLink {
            ty: "SIGNER".to_owned(),
            payload: format!("{signer_address:#x}"),
            signature: String::default(),
        });
        links.extend(delegates.iter().cloned());
        Self(links)
    }

    pub fn headers(&self) -> impl Iterator<Item = (String, String)> + '_ {
        self.0.iter().enumerate().map(|(ix, link)| {
            (
                format!("x-identity-auth-chain-{ix}"),
                serde_json::to_string(&link).unwrap(),
            )
        })
    }

    pub fn formdata(&self) -> impl Iterator<Item = (String, String)> + '_ {
        self.0.iter().enumerate().flat_map(|(ix, link)| {
            [
                (format!("authChain[{ix}][type]"), link.ty.clone()),
                (format!("authChain[{ix}][payload]"), link.payload.clone()),
                (
                    format!("authChain[{ix}][signature]"),
                    link.signature.clone(),
                ),
            ]
        })
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedLoginMeta {
    pub intent: String,
    pub signer: String,
    is_guest: bool,
    origin: String,
    /// A password-protected world's password
    #[serde(skip_serializing_if = "Option::is_none")]
    secret: Option<String>,
}

impl SignedLoginMeta {
    pub fn new(is_guest: bool, origin: Uri) -> Self {
        let origin = origin.into_parts();

        Self {
            intent: "dcl:explorer:comms-handshake".to_owned(),
            signer: "dcl:explorer".to_owned(),
            is_guest,
            origin: format!("{}://{}", origin.scheme.unwrap(), origin.authority.unwrap()),
            secret: None,
        }
    }

    pub fn with_secret(mut self, secret: Option<String>) -> Self {
        self.secret = secret;
        self
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod gateway_signing_tests {
    use super::*;
    use common::base_domain::{self, Service};

    #[test]
    fn engine_and_scene_sign_their_respective_paths() {
        base_domain::set_gateway("https://gateway.example.test").unwrap();
        let raw = base_domain::url(
            Service::CommsGatekeeper,
            "/Adapter/Mixed?case=Kept&search=two words",
        );
        let uri: Uri = reqwest::Url::parse(&raw).unwrap().as_str().parse().unwrap();
        assert_eq!(uri.host(), Some("gateway.example.test"));
        let mut wallet = Wallet::default();
        wallet.finalize_as_guest_with_seed([42; 32]);
        async_std::task::block_on(async {
            for path in [
                "/social-api/v2/communities/../../../auth-api/x",
                "/social-api/v2/communities/%2e%2E/%2e%2e/%2E%2e/auth-api/x",
                "/social-api/v2/communities/id%2f..%2f..%2f..%2f..%2fauth-api/x",
                "/social-api/v2/communities/id%5c..%5c..%5c..%5c..%5cauth-api/x",
                "/outside/../social-api/x",
            ] {
                let attack = format!("https://gateway.example.test{path}");
                assert!(
                    sign_service_request("GET", &attack, &wallet, "{}".into())
                        .await
                        .is_err(),
                    "{path}"
                );
            }

            for (headers, path) in [
                (
                    sign_service_request("POST", &raw, &wallet, "{}".into())
                        .await
                        .unwrap(),
                    "/adapter/mixed",
                ),
                (
                    sign_service_request(
                        "POST",
                        "https://notifications.example.test/old/../Mixed Name?search=two words",
                        &wallet,
                        "{}".into(),
                    )
                    .await
                    .unwrap(),
                    "/mixed%20name",
                ),
                (
                    sign_request("POST", &uri, &wallet, "{}".into())
                        .await
                        .unwrap(),
                    "/comms-gatekeeper/adapter/mixed",
                ),
            ] {
                let headers: std::collections::HashMap<_, _> = headers.into_iter().collect();
                let link: ChainLink =
                    serde_json::from_str(&headers["x-identity-auth-chain-1"]).unwrap();
                assert_eq!(
                    link.payload,
                    format!("post:{path}:{}:{{}}", headers["x-identity-timestamp"])
                );
            }
        })
    }
}

pub async fn sign_request(
    method: &str,
    uri: &Uri,
    wallet: &Wallet,
    meta: String,
) -> Result<Vec<(String, String)>, anyhow::Error> {
    sign_path(method, uri.path(), wallet, meta).await
}

/// Trusted engine/HUD request. Scene signing always uses exactly the path it supplied.
pub async fn sign_service_request(
    method: &str,
    raw: &str,
    wallet: &Wallet,
    meta: String,
) -> Result<Vec<(String, String)>, anyhow::Error> {
    // Gateway routing rejects ambiguous raw paths before URL normalization can erase them.
    let original = common::base_domain::gateway_signed_path(raw)?;
    let url = reqwest::Url::parse(raw)?;
    sign_path(
        method,
        original.as_deref().unwrap_or_else(|| url.path()),
        wallet,
        meta,
    )
    .await
}

async fn sign_path(
    method: &str,
    path: &str,
    wallet: &Wallet,
    meta: String,
) -> Result<Vec<(String, String)>, anyhow::Error> {
    let unix_time = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let payload = format!(
        "{}:{}:{unix_time}:{meta}",
        method.to_lowercase(),
        path.to_lowercase()
    );
    let auth_chain = wallet.sign_message(payload).await?;

    let mut headers: Vec<_> = auth_chain.headers().collect();
    headers.push(("x-identity-timestamp".to_owned(), format!("{unix_time}")));
    headers.push(("x-identity-metadata".to_owned(), meta));
    Ok(headers)
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;

    #[test]
    fn signing_snapshot_keeps_its_account_after_switch_and_logout() {
        let mut live = Wallet::default();
        live.finalize_as_guest();
        let original = live.address().unwrap();
        let snapshot = live.snapshot();
        live.finalize_as_guest();
        assert_ne!(live.address(), Some(original));
        live.disconnect();
        let chain = async_std::task::block_on(snapshot.sign_message("login".into())).unwrap();
        assert_eq!(chain.0[0].payload, format!("{original:#x}"));
        assert_eq!(snapshot.address(), Some(original));
        assert_eq!(live.address(), None);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod lock_tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

    #[test]
    fn account_readers_wait_for_an_account_update() {
        let wallet = Wallet::default();
        let mut update = wallet.0.write().unwrap();
        update.root_address = Some(Address::repeat_byte(7));
        update.guest_account = true;
        let (started, starting) = mpsc::channel();
        let (finished, finishing) = mpsc::channel();

        std::thread::scope(|scope| {
            scope.spawn(|| {
                started.send(()).unwrap();
                finished
                    .send((
                        wallet.address(),
                        wallet.is_guest(),
                        wallet.is_guest_account(),
                    ))
                    .unwrap();
            });
            starting.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(
                finishing.recv_timeout(Duration::from_millis(50)),
                Err(mpsc::RecvTimeoutError::Timeout)
            );
            drop(update);
            assert_eq!(
                finishing.recv_timeout(Duration::from_secs(5)).unwrap(),
                (Some(Address::repeat_byte(7)), true, true)
            );
        });
    }

    #[test]
    fn disconnect_waits_for_an_account_reader() {
        let mut wallet = Wallet::default();
        wallet.finalize_as_guest_with_seed([7; 32]);
        let mut disconnecting = wallet.clone();
        let reader = wallet.0.read().unwrap();
        let (started, starting) = mpsc::channel();
        let (finished, finishing) = mpsc::channel();

        std::thread::scope(|scope| {
            scope.spawn(move || {
                started.send(()).unwrap();
                disconnecting.disconnect();
                finished.send(()).unwrap();
            });
            starting.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(
                finishing.recv_timeout(Duration::from_millis(50)),
                Err(mpsc::RecvTimeoutError::Timeout)
            );
            drop(reader);
            finishing.recv_timeout(Duration::from_secs(5)).unwrap();
        });

        assert_eq!(wallet.address(), None);
        assert!(wallet.is_guest());
        assert!(!wallet.is_guest_account());
    }
}
