# Local development

Needs [Nix with flakes](https://nix.dev/install-nix) on Apple Silicon macOS,
Linux or WSL2 (clone outside `/mnt/c`).

```sh
git clone https://github.com/decentraland/dcl-one.git foundation  # GIT_LFS_SKIP_SMUDGE=1: skip docs media
cd foundation
nix develop                        # FOUNDATION_DEV_AUTOSTART=0: no services
nix develop --command make check   # non-interactive: never starts services
```

`nix develop` starts a per-checkout PostgreSQL 18, API, Sites, LiveKit, NATS and
Pulse that outlive the shell and reload on edits.

Ports, from base 5210 (`FOUNDATION_DEV_PORT` on first use; saved in
`.cache/dev/settings.json`): API and `/ws` +0, Sites and Pulse WebTransport +2,
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
- `testnet` (`FOUNDATION_DEV_PROFILE=testnet`) mirrors the Shop's catalogue and order book from Decentraland's testnet (.zone, Amoy and Sepolia) and settles on Amoy, for [`tooling/shop-trade-journey`](../tooling/shop-trade-journey/README.md). Start it in its own worktree: a stack holding data of another profile refuses to switch to it, and a testnet stack refuses `trades-snapshot`. Credits settle through Amoy's CreditsManager with mock card top-ups; the stack signs them only when `.cache/dev/credits-signer.key` holds a key with that contract's signer role.
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

### Pull request previews

`Web previews / Build` selects two independent products: changes under `apps/sites/` build Sites, and changes under `library/ui/` build its standalone UI reviewer. API-only changes do not select previews. Changes to the preview pipeline select both products. The workflow evaluates every PR update against its cumulative diff so later commits keep the selected preview current; PRs with no selected products run no product build; if an earlier preview comment exists, it is updated to say no products are selected at the current commit.

Sites and its current UI dependency declare `pnpm@10.19.0` in their package manifests, matching the UI reviewer. Corepack resolves each manager from the respective working directory.

The workflow runs `pnpm review:build --base ./` from `library/ui`, invoking Vite with `figma/review/review.vite.config.ts` and producing `library/ui/figma/review/review-dist/`. Versioned CDN navigation, references, render images and JSON fetches still require the pending source change and an exact source commit passing the nested-prefix regression. Publication is disabled unless the repository variable `WEB_PREVIEW_REVIEWER_ENABLED` equals `true`; enable it only after that regression and source audit pass. Builds and artifacts remain available while publication is disabled.

`Web previews / Publish` uses trusted publication code and validated static artifacts, without running pull request lifecycle scripts with credentials. Same-repository, open pull requests receive one updated status comment; forks build artifacts without publishing. A reviewer link appears only after the CDN serves the matching source receipt, HTML entry, a version-scoped hashed asset and fidelity JSON.

Publication needs `GITLAB_CDN_DEPLOYER_TOKEN` and `GITLAB_CDN_DEPLOYER_URL`, as used by Bevy, and `FIGMA_FILE_KEY` for the trusted artifact key scan. The reviewer and its dependency-free key guard must already be on the default branch before publication can run. The trusted scan uses `node library/ui/figma/key-guard.mjs --no-worktree --no-range --path .preview/input`. No Figma key is passed to the PR build. The public npm package `@dcl-regenesislabs/dcl-one-ui-review` needs a trusted publisher for this repository and `web-previews-publish.yml` using npm trusted publishing. Publication uses timestamp versions so a retry can publish after a previous npm upload succeeded. Registry polling is limited to 20 attempts at 10-second intervals, leaving time for CDN verification within the publication job timeout. The version's commit suffix identifies trusted publication code; `preview-source.json` records the built PR head and source build run. GitHub artifacts expire after seven days; this does not delete immutable npm versions or CDN copies.

To withdraw a reviewer preview, first disable `WEB_PREVIEW_REVIEWER_ENABLED` to stop new publications. Record the affected package version, source receipt and complete preview URL, then request withdrawal from the npm/CDN operators and a cache purge for that version prefix. Removing a PR dist-tag or deprecating an npm version does not remove cached HTML or JavaScript. Confirm that the affected entry and bundle URLs no longer serve the content before considering the withdrawal complete; this workflow has no automated teardown or verified operator API for that operation.

Sites uploads only `apps/sites/build/` as `web-preview-sites` for inspection. This artifact has no automated deployment consumer and is not a complete runtime package. The planned preview uses a container image built from the pull request and runs in Kubernetes at a shared hostname supplied by deployment configuration. It is manually triggered and expires after six hours. The infrastructure owner supplies the cluster configuration; application deployment and the dev/prod service selector are not implemented. The PR comment reports that the preview is pending.

Publisher jobs serialize per PR and comment jobs serialize per source branch using GitHub's `queue: max`, preserving pending runs while eligibility checks discard obsolete work. GitHub API reads and idempotent PATCH requests retry transient failures at most three times; comment-creation POSTs are not retried because the first request might already have created the comment. Source validation has a five-minute job timeout. PR association uses the PR number, base ref and head SHA; advancing the base tip alone does not invalidate a build of the same head.

### Local checks

```sh
nix develop --command bash tooling/dev-smoke.sh
nix develop --command bash tooling/dev-test.sh [rust|e2e]   # rust: PostgreSQL only
```

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
