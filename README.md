# dcl-one

Decentraland services, web apps and tooling: the source that builds and serves
the website, sign-in, profiles, Play, the Creator Hub, the protocol and the
scene SDK toolchain, with the test suites that run from the tree alone and the
development environment. End-to-end suites that need a live stack, the API
conformance capture corpus and internal docs stay in the private repository.

## Layout

| Path | Contents |
| --- | --- |
| `services/catalyst/` | Rust workspace: the `catalyst` binary (APIs, content, comms, worlds, shop, social) |
| `apps/sites/`, `apps/ui3/` | Web sites; shared product UI |
| `apps/dcl-scene-cli/` | The scene SDK toolchain (`dcl-scene-cli`) |
| `apps/creator-hub/` | Creator Hub native shell |
| `rendering/bevy-explorer/` | Explorer source and Play pins |
| `apps/bevy-explorer/` | Explorer builds and images |
| `library/` | Shared crates, API types, the protocol (`library/protocol`) |
| `schemas/` | The PostgreSQL schema |
| `infra/` | Nix packages, container images, the gateway |
| `tooling/` | Build and publication tools used by the workflows, developer tools such as the Shop parity harness, and their tests |

## Agentic roles

Per the [implementation pipeline][roles]; the user accepts decisions and
approves merges. Start specialist sessions in `services/catalyst/`.

| Role | Goal | Main functions |
| --- | --- | --- |
| [Main session][roles] | Coordinate delivery | Plan, dispatch, combine reviews, record decisions, reconcile evidence |
| [Implementer][roles] | Deliver a work package | Code and tests in a worktree; report architecture changes |
| [`comms-architect`][arch] | Simple, coherent architecture | Validate pinned source and tracked designs; report proposed decisions |
| [`comms-ci-engineer`][ci] | Trustworthy gates | Build checks; review installed gates; report proposed CI updates |
| [Code reviewer][roles] | Correctness | Fresh-context PR review |
| [`divergence-analyst`][div] | Compatibility | Compare pinned source per client version; report findings and proposed conformance checks |
| [`comms-security-reviewer`][roles] | Exploitable comms behavior | Review identity, tokens, admission, bans, rate limits |

[roles]: services/catalyst/docs/comms/implementation-pipeline.md#roles
[arch]: services/catalyst/.claude/agents/comms-architect.md
[ci]: services/catalyst/.claude/agents/comms-ci-engineer.md
[div]: services/catalyst/.claude/agents/divergence-analyst.md

## Build

Needs Python 3.12+, Rust per `services/catalyst/rust-toolchain.toml`, Node 24+
and pnpm. Images build with Nix (`nix build .#image-apis`, `.#image-sites`,
`.#image-gateway`; see [images](infra/containers/README.md)).

```sh
make catalyst-build sites-install sites-build
make bevy-web   # or bevy-headless
```

## Test

`make catalyst-test` runs the service workspace; the suites that touch a database
or the message bus read `CATALYST_TEST_DATABASE_URL` (a PostgreSQL the tests may
create databases in), `PULSE_NATS_SERVER_BIN` and `ARCHIPELAGO_NATS_SERVER_BIN`,
and skip when unset. The web packages run `pnpm test` (`apps/ui3`, `apps/sites`,
`apps/ui3/editor-scene`) and `npm test` in `rendering/bevy-explorer/source/react-web`
after `scripts/gen-ts-bindings.sh`; the protocol runs `make -C library/protocol
test`; the workflow guards run with `python3 -m unittest discover -s tooling/tests`.

## Standards

Web UI code follows [UI standards](docs/ui-standards.md); component styles use
CSS Modules.

## Clone the repository

```sh
gh repo clone decentraland/dcl-one && cd dcl-one
git lfs install --local && git lfs pull
```

Root code is [AGPL-3.0](LICENSE); the Explorer keeps Apache-2.0.

Contributor contracts: [AGENTS.md](AGENTS.md), [DATABASE.md](DATABASE.md). Engineering references and [ADRs](docs/README.md) are tracked; working reports and private ledgers stay outside the public tree.
