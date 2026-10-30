//! The old crate's `tests/` directory, file by file, as lib tests over the
//! process's scratch database (`CATALYST_TEST_DATABASE_URL`; skipped
//! without it) and a throwaway blob store.

pub(crate) mod support;

mod curation_auth;
mod curation_auth_fuzz;
mod draft_persistence;
mod linked_publication;
mod projects;
mod publication_preparation;
mod publication_transactions;
