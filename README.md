# dcl-one

[![SDK](https://img.shields.io/npm/v/@dcl/sdk?label=sdk)](https://www.npmjs.com/package/@dcl/sdk)

Decentraland services, web apps and tooling: the source that builds and serves
the website, sign-in, profiles, Play, the Creator Hub, the protocol and the
scene SDK toolchain, with the test suites that run from the tree alone and the
development environment. End-to-end suites that need a live stack, the API
conformance capture corpus and internal docs stay in the private repository.

## Layout

| Path | Contents |
| --- | --- |
| `services/catalyst/` | Rust workspace: the `catalyst` binary (APIs, content, comms, worlds, shop, social) |
| `apps/sites/` | Web sites |
| `apps/dcl-scene-cli/` | The scene SDK toolchain (`dcl-scene-cli`) |
| `apps/creator-hub/` | Creator Hub desktop: npm workspace with the Electron app, `@dcl/inspector` and `@dcl/asset-packs` ([ADR](docs/adr/2026-11-09-creator-hub-in-dcl-one.md)) |
| `rendering/bevy-explorer/` | Explorer source and Play pins |
| `apps/bevy-explorer/` | Explorer builds and images |
| `library/` | Shared crates, API types, the protocol (`library/protocol`), the SDK (`library/js-sdk-toolchain`) |
| `schemas/` | The PostgreSQL schema |
| `infra/` | Nix packages, container images, the gateway |
| `tooling/` | Build and publication tools used by the workflows, developer tools such as the Shop parity harness, and their tests |

## Agentic roles

Per the [implementation pipeline][roles]; the user accepts decisions and
approves merges. Start specialist sessions in `services/catalyst/`, and Creator Hub
sessions in `apps/creator-hub/`.

| Role | Goal | Main functions |
| --- | --- | --- |
| [Main session][roles] | Coordinate delivery | Plan, dispatch, combine reviews, record decisions, reconcile evidence |
| [Implementer][roles] | Deliver a work package | Code and tests in a worktree; report architecture changes |
| [`comms-architect`][arch] | Simple, coherent architecture | Validate pinned source and tracked designs; report proposed decisions |
| [`comms-ci-engineer`][ci] | Trustworthy gates | Build checks; review installed gates; report proposed CI updates |
| [Code reviewer][roles] | Correctness | Fresh-context PR review |
| [`divergence-analyst`][div] | Compatibility | Compare pinned source per client version; report findings and proposed conformance checks |
| [`comms-security-reviewer`][roles] | Exploitable comms behavior | Review identity, tokens, admission, bans, rate limits |
| [`dcl-ch-feature-architect`][ch-arch] | Coherent cross-layer Creator Hub changes | Plan a change across asset packs, inspector, app and SDK; dispatch the specialists; verify the layers together |
| [`dcl-asset-packs-specialist`][ch-ap] | `@dcl/asset-packs` changes | Smart items, schemas, packs and catalog; a packed tarball for the next layer |
| [`dcl-inspector-specialist`][ch-insp] | `@dcl/inspector` changes | Editor, panels, UI Designer, Bevy bridge, data layer; a packed tarball for the SDK |
| [`dcl-creator-hub-specialist`][ch-app] | Creator Hub app changes | Electron main, preload and renderer |
| [`dcl-sdk-toolchain-specialist`][ch-sdk] | Inspector builds reach scenes | Wire a local inspector into `@dcl/sdk-commands`; prove a previewed scene runs it |
| [`dcl-designer-specialist`][ch-design] | Figma-faithful UI | Pull Figma frames into specs, sync tokens, validate renders |
| [`dcl-creators-data-specialist`][ch-data] | Analytics metrics backend | Admit metrics in creators-data; run a local instance for the Analytics page |

[roles]: services/catalyst/docs/comms/implementation-pipeline.md#roles
[arch]: services/catalyst/.claude/agents/comms-architect.md
[ci]: services/catalyst/.claude/agents/comms-ci-engineer.md
[div]: services/catalyst/.claude/agents/divergence-analyst.md
[ch-arch]: apps/creator-hub/.claude/agents/dcl-ch-feature-architect.md
[ch-ap]: apps/creator-hub/.claude/agents/dcl-asset-packs-specialist.md
[ch-insp]: apps/creator-hub/.claude/agents/dcl-inspector-specialist.md
[ch-app]: apps/creator-hub/.claude/agents/dcl-creator-hub-specialist.md
[ch-sdk]: apps/creator-hub/.claude/agents/dcl-sdk-toolchain-specialist.md
[ch-design]: apps/creator-hub/.claude/agents/dcl-designer-specialist.md
[ch-data]: apps/creator-hub/.claude/agents/dcl-creators-data-specialist.md

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
and skip when unset. The test binaries run `CATALYST_TEST_JOBS` at a time
(default: the core count, at most 16); tests create their own scratch databases, and
the socket binaries run with at most one test thread per core. The web
packages run `pnpm test` (`apps/ui3`, `apps/sites`, `apps/ui3/editor-scene`) and
`npm test` in `rendering/bevy-explorer/source/react-web` after
`scripts/gen-ts-bindings.sh`; the protocol runs `make -C library/protocol
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
