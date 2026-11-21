//! Hashes, signatures, principals and auth chains (copied from
//! catalyrst-crypto, catalyrst-auth-chain and catalyrst-hashing).
//!
//! `hash` is the content-addressing side (CIDv0 `Qm...`, CIDv1 `bafy...`,
//! canonical-CID checks); `auth_chain`/`recover`/`verify`/`eip1654` are the
//! `@dcl/crypto` auth-chain port; `eth_address`, `hex0x` and `pointer` are the
//! wire primitives shared with them. `rpc_validator`/`validation_cache` are the
//! write-path EIP-1654 contract-wallet check (JSON-RPC `isValidSignature` behind
//! a positive-result cache) that the write deployer plugs into `verify`.
//! `request` is the signed-request verifier (`x-identity-*`) every surface moves
//! onto: HTTP routes, socket auth frames and the Pulse handshake;
//! `signed_fetch` keeps the steps the surfaces not yet on it assemble.
//! `scoped_login` is the scoped delegation (ADR-162) and `js_date` the
//! `Date.parse` its expirations are read with.

pub mod admin_bearer;
pub mod auth_chain;
mod cid;
pub mod eip1654;
pub mod eip712;
pub mod error;
pub mod eth_address;
pub mod hash;
pub mod hex0x;
pub mod js_date;
pub mod pointer;
pub mod recover;
pub mod request;
pub mod rpc_validator;
pub mod scoped_login;
pub mod signed_fetch;
mod unixfs;
pub mod validation_cache;
pub mod verify;
pub mod wallet;

pub use auth_chain::{
    AuthChain, AuthLink, AuthLinkType, MAX_AUTH_CHAIN_LINKS, MAX_AUTH_LINK_FIELD_LEN,
    is_valid_auth_chain, parse_ephemeral_payload,
};
pub use eip1654::{Eip1654Validator, PeerValidated, verify_eip1654};
pub use error::AuthError;
pub use eth_address::{EthAddress, is_eth_address, normalize_eth_address};
pub use hash::{
    HashV1Writer, hash_bytes, hash_bytes_v0_unixfs, hash_bytes_v1, is_canonical_cid,
    raw_cid_v1_from_sha256_hex, verify_hash,
};
pub use hex0x::{HexDecodeError, decode_hex_0x};
pub use pointer::{canonicalize_pointer, is_canonical_pointer, parse_pointer};
pub use recover::{recover_address, recover_address_from_digest};
pub use rpc_validator::{RpcEip1654Validator, rpc_client};
pub use validation_cache::{ValidationCache, cached_rpc_validator};
pub use verify::{
    LOGIN_PURPOSE, verify_auth_chain, verify_auth_chain_async, verify_login_chain,
    verify_login_chain_async,
};
pub use wallet::{SignError, Wallet, create_scoped_auth_chain, create_simple_auth_chain};
