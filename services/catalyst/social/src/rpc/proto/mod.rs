//! The social service messages: `decentraland.social_service.v2` (the RPC
//! payloads and stream updates), `decentraland.social_service` (the shared
//! error messages) and `decentraland.common` (`Color3`), compiled by build.rs
//! from the one proto source (`library/protocol/proto`) into OUT_DIR, one
//! module per package and no service generator. The module tree keeps the
//! package nesting (`decentraland::social_service::v2` reaches `Color3` as
//! `super::super::common::Color3`).

#[allow(clippy::all)]
pub mod decentraland {
    pub mod common {
        include!(concat!(env!("OUT_DIR"), "/decentraland.common.rs"));
    }
    pub mod social_service {
        include!(concat!(env!("OUT_DIR"), "/decentraland.social_service.rs"));
        pub mod v2 {
            include!(concat!(
                env!("OUT_DIR"),
                "/decentraland.social_service.v2.rs"
            ));
        }
    }
}

pub use decentraland::common;
pub use decentraland::social_service as errors;
pub use decentraland::social_service::v2;
