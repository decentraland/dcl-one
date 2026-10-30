//! Admission: who may join which room. The scene, server-scene, world and
//! island tokens (`handlers`, `world_mint`, `cluster`), the participant
//! removal of the `/ws` connector's kicks (`island_kick`), the Pulse room
//! authority (`relay_authority`) and the island refresh (`island_refresh`),
//! over the scene and world identity of `scene`.

pub mod cluster;
pub mod handlers;
pub mod island_kick;
pub mod island_refresh;
pub mod relay_authority;
pub mod scene;
pub mod world_mint;
