# ADR: Organizing `library/`, `services/` and the top-level folders

- Status: Accepted
- Date: 2026-10-19

## Decision

Each top-level folder holds one kind of content: `apps/` what users run,
`library/` shared code, `services/` the backend. A crate's `src/` is its module;
crates alias each other by module name (`use catalyst_db as db;`) and the root
`lib.rs` re-exports them.
`services/catalyst/scripts/check-module-dag.sh` enforces library isolation and
[`module-layers.toml`](../module-layers.toml) (a module uses only lower layers,
never a peer).

## Rules for new code

- Each new folder or crate serves one of the ten deliverables.
- A new crate needs an independent consumer or a concrete build requirement; otherwise it is a module.
- Put a crate in `library/` (named `dcl-*`) only if several deliverables use it and it depends on nothing in `services/` or `apps/`; add it to `[workspace.dependencies]` and the `library/README.md` table.
- A domain crate is `services/catalyst/<module>`, package `catalyst-<module>`: add it to `members` and `docs/module-layers.toml`, alias its dependencies in `lib.rs`, re-export it from the root `lib.rs`.
- Fix a back-edge with a trait the lower layer owns, not an `[allow]` line.
- Name other files from `DCL_ONE_ROOT`, never `../` chains.
- Keep external code's license and lock beside it.
- Add every new path to an `ownership.json` area.
