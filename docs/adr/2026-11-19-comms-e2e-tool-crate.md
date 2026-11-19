# ADR: `comms-e2e`, a tool crate beside the Catalyst domain crates

- Status: Accepted
- Date: 2026-11-19

## Context

The [layout ADR](2026-10-19-organizing-library-services-folders.md) describes one kind of
crate under `services/catalyst/`: a domain crate, aliased and re-exported by the root
`lib.rs` and linked into the node.

End-to-end scenarios against a running comms stack need a program of their own: protocol
bots for Pulse, the archipelago and LiveKit, a relay that injects network faults, and a
driver for the headless Explorer. What it expects must be what the servers enforce (wire
types, limits, disconnect codes), and nothing the node ships may contain it.

## Decision

`services/catalyst/comms-e2e` (package `catalyst-comms-e2e`, binary `comms-e2e`) is a tool
crate in the Catalyst workspace.

- It depends on `catalyst-comms` with the `test-support` feature and on `library/` crates.
  No crate depends on it, and the root `lib.rs` does not re-export it.
- It is a workspace member: it resolves the servers' lock file and reads their constants
  at the same commit.
- `docs/module-layers.toml` lists it above `comms`, the one domain crate it uses.
- The node is built with `--bin catalyst`, so neither the tool nor `test-support` is in
  what is deployed.
- `ownership.json` and `CODEOWNERS` name its path.

## Alternatives considered

- **A module of `catalyst-comms`.** The bots, the fault relay and the Explorer driver, and
  their dependencies, would compile into the server crate.
- **A workspace of its own under `tooling/`.** It would resolve its own versions of the
  servers' dependencies and could drift from the code it checks.
- **`library/`.** That folder holds code several deliverables share and that depends on
  nothing in `services/`.

## Consequences

- `cargo test` in the Catalyst workspace builds the tool and runs its own tests, which
  start a Pulse server inside the test process.
- A change to a server constant or wire type the tool uses fails the tool's build or tests
  in the same pull request.
- `services/catalyst/` now holds a second kind of crate: a tool that nothing links. The
  layout ADR lists it as an exception to the domain-crate rule.
