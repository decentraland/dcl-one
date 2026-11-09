# Local development

Needs [Nix with flakes](https://nix.dev/install-nix) on Apple Silicon macOS,
Linux or WSL2 (clone outside `/mnt/c`).

```sh
git clone https://github.com/decentraland/dcl-one.git foundation  # GIT_LFS_SKIP_SMUDGE=1: skip docs media
cd foundation
nix develop                        # FOUNDATION_DEV_AUTOSTART=0: no services
nix develop --command make check   # non-interactive: never starts services
```

`flake.nix` pins nixpkgs to a nixos-unstable commit. The Update nixpkgs workflow
proposes the current tip on the first of each month; `python3
tooling/nixpkgs-update.py` does the same locally.

`nix develop` starts a per-checkout PostgreSQL 18, API, Sites, LiveKit, NATS and
Pulse that outlive the shell and reload on edits.

Ports, from base 5210 (`FOUNDATION_DEV_PORT` on first use; saved in
`.cache/dev/settings.json`): API and `/ws` +0, its metrics +1, Sites and Pulse WebTransport +2,
PostgreSQL +3, LiveKit +4..+6, Pulse ENet and `/metrics` +7, NATS +8, its
monitor +9. An occupied saved port fails startup.

| `foundation-dev …` | Action |
| --- | --- |
| `status` | URLs, health, Pulse SPKI hash |
| `wait --timeout 1800` | Wait for readiness |
| `logs`, `restart <service>` | After lockfile changes restart `web-install`, then `sites` |
| `up`, `down` | Start or attach, stop (`--postgres-only`) |
| `profile mirror\|testnet\|offline` | Switch profile while stopped |
| `service install\|uninstall` | Login service (WSL: `systemd=true`) |
| `trades-snapshot <file>` | Snapshot for `FOUNDATION_DEV_TRADES_SNAPSHOT` |
| `env` | App URLs, `FOUNDATION_DEV_TEST_DATABASE_URL`, `FOUNDATION_DEV_STACK_PROFILE` |

Without this Chrome profile, `/play` logs `pulse: webtransport connect failed`
and shows no other avatars:

```sh
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --user-data-dir="$(mktemp -d)" \
  --webtransport-developer-mode --ignore-certificate-errors-spki-list=<hash from status>
```

`/about` answers 503 while Pulse is down.

## Data profiles

- `mirror` (default) mirrors catalogues hourly and reads content and worlds through on a miss; places, worlds and events answer 503 until their first pass.
- `WORLDS_READ_THROUGH_TRUST_UPSTREAM_GRANTS` trusts the upstream's deployer list; it is not verification.
- Deployments answer 503 except profiles (`SERVE_UNSYNCED_DEPLOY_TYPES=profile`); nothing is written upstream.
- `offline` (`FOUNDATION_DEV_PROFILE=offline`) starts empty for local deployments.
- `testnet` (`FOUNDATION_DEV_PROFILE=testnet`) mirrors the Shop's catalogue and order book from Decentraland's testnet (.zone, Amoy and Sepolia) and settles on Amoy, for [`tooling/shop-trade-journey`](../tooling/shop-trade-journey/README.md). Start it in its own worktree: a stack holding data of another profile refuses to switch to it, and a testnet stack refuses `trades-snapshot`. Credits settle through dcl-one's own CreditsManager on Amoy with mock card top-ups (the Shop refuses the shared upstream one); the stack signs them only when `.cache/dev/credits-signer.key` holds a key with that contract's signer role. Until the relay admits that manager, a credits purchase is a transaction the wallet pays gas for, so in-app wallets can't buy with Credits on this profile.
- Never delete `.cache/dev` to fix a migration failure.
- Keep `settings.json` with the database; without it startup refuses to create credentials.
- Use Linux for exact `en_US.utf8` collation comparisons.

## The web Explorer (`/play`)

`web-install` imports our verified Bevy web build (`/play/sites-play.json`):

```sh
gh auth login   # import CI artifacts, or build locally:
make bevy-web && python3 tooling/fetch-play.py --from-build apps/bevy-explorer/dist/web
gh workflow run bevy-explorer-ci.yml --ref main   # no matching or expired artifact
```

See [production rules](../infra/containers/README.md#sites-explorer-build-input).
`/play/app.html?mock=1` runs the HUD without the WebGPU engine.

## Validation

```sh
nix develop --command bash tooling/dev-smoke.sh
nix develop --command bash tooling/dev-test.sh [rust|e2e]   # rust: PostgreSQL only
```

CI runs the Rust suites in `nix develop .#ci`: the toolchain, the services the tests
start and the build tools, without the web and media tools.

Run both in a fresh checkout. `python3 tooling/ci-local.py install` adds hooks
that run the matching workflows' checks (images need Linux or
`CI_LOCAL_IMAGES=1`); `ci-local.py run [REV]` runs them without pushing.
On PRs, macOS runs the Rust tests only for build and dev-stack inputs: Cargo
manifests and lockfiles, build scripts, toolchain files, `.cargo`,
`library/vendor/rusty_enet`, Nix, `infra/dev`, the Makefile, the dev tooling and
`.github/workflows/development.yml` (`tooling/dev-ci.py`). Catalyst, library and
schema source, the web apps and other Python tooling stay on Linux. On `main`,
the Rust tests and smoke run on macOS, and a new push does not cancel a running
Development run: it waits, replacing any run already waiting, so each completed
run tests every earlier commit and a macOS-only failure in source shows up
there. A PR labelled `ci:macos` runs both on macOS (label, then push or reopen;
re-running a job does not pick up a new label).

Linux jobs run on `ubuntu-26.04` unless they compile the explorer or uuav natively or
test what they compiled: those run on `ubuntu-24.04`, the image the uuav binaries lock
records and the glibc 2.39 floor of the shipped Linux desktop, headless and uuav
binaries; the 16-core Linux runner exists only as `ubuntu-24.04-16core`. macOS jobs run
on `macos-15`, Windows jobs on `windows-2025`; `xlarge` and `16core` where the
explorer's test suite and headless packages need them.

## CI runners

Every `runs-on` resolves through a repository variable `RUNNER_{OS}_{ARCH}_{VERSION}_{SIZE}`
that falls back to the GitHub-hosted label written next to it. Defaults are left out of the
name: ARCH for Windows (x86) and macOS (arm), while Linux always writes `X86` or `ARM`;
VERSION for Ubuntu 26.04, macOS 15 and Windows 2025; SIZE for the standard runner, otherwise
the label's own word (`16CORE`, `XLARGE`). So `ubuntu-26.04` is `RUNNER_LINUX_X86`,
`ubuntu-24.04-16core` is `RUNNER_LINUX_X86_UBUNTU_24_04_16CORE` and `macos-15-xlarge` is
`RUNNER_MACOS_XLARGE`. Unset, the job runs on the hosted label. Set to one of a fork's own
runner labels, every job of that class runs there with the same workflow files, so a fork
chooses per OS, architecture, version and size which jobs it hosts and which stay on GitHub.
`BEVY_E2E_WINDOWS_RUNNER` stays the separate switch for the GPU scenarios job, and the
benchmark legs keep their laptop labels from `tooling/benchmark-inputs.py`.

| Variable | Hosted label | Jobs | Workflows |
| --- | --- | ---: | --- |
| `RUNNER_LINUX_X86_UBUNTU_24_04` | `ubuntu-24.04` | 6 | bevy-explorer-ci, bevy-explorer-desktop, bevy-explorer-headless-build, bevy-explorer-scenes, uuav-native, uuav-verify |
| `RUNNER_LINUX_X86_UBUNTU_24_04_16CORE` | `ubuntu-24.04-16core` | 3 | bevy-explorer-ci (3) |
| `RUNNER_LINUX_X86` | `ubuntu-26.04` | 54 | apis-sites-images (5), bevy-explorer-benchmark (6), bevy-explorer-ci (5), bevy-explorer-ci-status (3), bevy-explorer-desktop, bevy-explorer-e2e (3), bevy-explorer-headless (3), bevy-explorer-images (3), bevy-explorer-rollout, bevy-explorer-rollout-manual, bevy-explorer-scenes (2), bevy-explorer-upstream, closed-pr-cache, development (5), gateway-images (2), nixpkgs-update, pr-labeler, protocol, sdk (2), social-captures, social-voice-release, structure (2), uuav-native (2), uuav-verify |
| `RUNNER_LINUX_ARM` | `ubuntu-26.04-arm` | 3 | apis-sites-images, bevy-explorer-images, gateway-images |
| `RUNNER_WINDOWS` | `windows-2025` | 4 | bevy-explorer-desktop, bevy-explorer-e2e, uuav-native, uuav-verify |
| `RUNNER_WINDOWS_16CORE` | `windows-2025-16core` | 2 | bevy-explorer-ci, bevy-explorer-headless-build |
| `RUNNER_MACOS` | `macos-15` | 4 | development (2), uuav-native, uuav-verify |
| `RUNNER_MACOS_XLARGE` | `macos-15-xlarge` | 2 | bevy-explorer-ci, bevy-explorer-headless-build |

```bash
for v in RUNNER_LINUX_X86_UBUNTU_24_04 RUNNER_LINUX_X86; do
  gh variable set "$v" --body my-linux-runner
done
```
