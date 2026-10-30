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
| `tooling/` | Build and publication tools used by the workflows, and their tests |

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

## Clone the repository

```sh
gh repo clone decentraland/dclone && cd dclone
git lfs install --local && git lfs pull
```

Root code is [AGPL-3.0](LICENSE); the Explorer keeps Apache-2.0.
