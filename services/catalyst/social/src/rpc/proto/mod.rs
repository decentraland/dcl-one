//! The social service messages: `decentraland.social_service.v2` (the RPC
//! payloads and stream updates), `decentraland.social_service` (the shared
//! error messages) and `decentraland.common` (`Color3`). The `.proto` files
//! beside this module are the contract unity-explorer compiles against
//! (decentraland/protocol, vendored by social-service-ea); `generated/*.rs`
//! is the prost 0.14.4 output for them, produced once by a scratch crate
//! (protox 0.9.1 `compile(files, ["."])` + `prost_build::Config::new()
//! .compile_fds(fds)`, no service generator) and committed so the build
//! needs no protoc and no build-time codegen. Regenerate the same way when a
//! `.proto` changes; the module tree below must keep the package nesting
//! (`decentraland::social_service::v2` reaches `Color3` as
//! `super::super::common::Color3`).

#[allow(clippy::all)]
pub mod decentraland {
    pub mod common {
        include!("generated/decentraland.common.rs");
    }
    pub mod social_service {
        include!("generated/decentraland.social_service.rs");
        pub mod v2 {
            include!("generated/decentraland.social_service.v2.rs");
        }
    }
}

pub use decentraland::common;
pub use decentraland::social_service as errors;
pub use decentraland::social_service::v2;
