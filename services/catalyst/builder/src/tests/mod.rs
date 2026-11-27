//! The builder's use cases through its router and store, as lib tests over the
//! process's scratch database (`CATALYST_TEST_DATABASE_URL`; skipped
//! without it) and a throwaway blob store.

pub(crate) mod support;

mod chain_state;
mod collection_writes;
mod cors;
mod curations;
mod draft_persistence;
mod item_files;
mod item_videos;
mod items;
mod limits;
mod newsletter;
mod publication_preparation;
mod publication_transactions;
mod small_reads;
mod upstream_routes;
mod validator;
mod wire_contract;
mod write_gate;
