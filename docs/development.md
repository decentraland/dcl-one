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

### Pull request previews

`Web previews / Build` selects two independent products: changes under `apps/sites/` build Sites, and changes under `library/ui/` build its standalone UI reviewer. API-only changes do not select previews. Changes to the preview pipeline select both products. The workflow evaluates pushes and base retargeting against the cumulative PR diff; title/body edits skip selection. Later documentation-only commits keep an already selected preview current; PRs with no selected products run no product build; if an earlier preview comment exists, it is updated to say no products are selected at the current commit.

Sites and its current UI dependency declare `pnpm@10.19.0` in their package manifests, matching the UI reviewer. Corepack resolves each manager from the respective working directory.

Before installing dependencies or building, the reviewer job runs the dependency-free `lock-digests.mjs` and `port-verification.mjs` checks from `library/ui/figma/`, validating the exact PR head and its merge base with the default branch. They establish tracked-file digest and port-record structural consistency, not cryptographic attestation that reported checks ran or content was accepted; CODEOWNERS review and an independent Audit naming the exact source SHA supply the review authority. The workflow runs `pnpm review:build --base ./` from `library/ui`, invoking Vite with `figma/review/review.vite.config.ts` and producing `library/ui/figma/review/review-dist/`. It then runs the source-owned `node library/ui/figma/review/scripts/check-build.mjs library/ui/figma/review/review-dist --prefix /@scope/package/0.1.0-preview/` before uploading. Chromium checks the live catalog, component navigation, variant selection, a component render with a nonzero box and runtime resources under the nested prefix, without page or console errors. The gallery publishes no visual-comparison interface or captured comparison evidence and makes no visual-parity claim. Actual product icons and assets remain supported; approved mock imagery may use disclosed placeholders, which is not a generic image-format ban. The build summary explains product failures, disabled publication and pending SSR hosting independently of publisher installation. Publication remains disabled until the [activation checklist](adr/2026-11-05-web-pr-previews.md#activation) is verified and `WEB_PREVIEW_REVIEWER_ENABLED` is set to `true`.

`Web previews / Publish` uses code and the static key guard from its trusted default-branch workflow commit. It validates static artifacts, rejects retired comparison output and writes its own package metadata without lifecycle scripts; it never runs the PR gallery, source-owned browser checker or other PR source with publication credentials. Same-repository, open pull requests receive one updated status comment; forks build artifacts without publishing. Reviewer publication runs in the `web-preview` environment, which must require a human reviewer and disable administrator bypass. The job checks those protections and the exact default-branch allowlist before handling artifacts and immediately before publishing. A pending status comment precedes the approval wait and replaces any older preview link; a configured maintainer may approve a build they authored. After approved publication, the comment reports upload in progress, an expected duration of about 6-10 minutes and a link to Actions for troubleshooting. A newer PR head or source-run attempt invalidates queued approval. A successful `cdn` / `upload-to-cdn` deployment for the package/version triggers a separate completion job, which posts a reviewer link only after verifying the matching source receipt, static HTML entry and a version-scoped hashed runtime asset.

Publication uses the shared organization secrets `GITLAB_CDN_DEPLOYER_TOKEN` and `GITLAB_CDN_DEPLOYER_URL`, retaining their existing Bevy consumers, and the protected `web-preview` environment secret `FIGMA_FILE_KEY` for the trusted artifact key scan. The trusted publisher and its dependency-free key guard must be installed on the default branch before publication can run; the gallery itself can remain in an open feature PR. The trusted scan uses `node library/ui/figma/key-guard.mjs --no-worktree --no-range --artifact .preview/input`. No Figma key is passed to the PR build. Activation remains pending verification of package registration in `definitions/src/static-site-rollouts.ts` and publication of `@decentraland/definitions@next`, initial npm package creation, and npm trusted publishing for `@dcl-regenesislabs/dcl-one-ui-review` bound to this repository, `web-previews-publish.yml` and the `web-preview` environment. The GitLab upload pipeline must pin `https://registry.npmjs.org` and reject or ignore caller-selected alternate registries; otherwise a matching source receipt does not authenticate its uploader. Runtime environment checks cannot attest to secret storage, npm configuration or the GitLab registry policy. Publication uses timestamp versions; retrying publication creates another immutable npm version. Npm registry polling is limited to 20 attempts at 10-second intervals. After the CDN upload success signal, transient receipt and runtime-resource failures share a 60-second retry window; source-identity mismatches and URLs escaping the version prefix fail immediately. Retrying that completion job checks the same version without republishing. The version commit suffix identifies trusted publication code; `preview-source.json` records the built PR head, source run and attempt. Re-runs replace the named build artifact, and an earlier attempt cannot reuse approval for a later attempt. GitHub artifacts expire after seven days; this does not delete immutable npm versions or CDN copies.

To withdraw a reviewer preview, first disable `WEB_PREVIEW_REVIEWER_ENABLED` to stop new publications. Record the affected package version, source receipt and complete preview URL, then request withdrawal from the npm/CDN operators and a cache purge for that version prefix. Removing a PR dist-tag or deprecating an npm version does not remove cached HTML or JavaScript. Confirm that the affected entry and bundle URLs no longer serve the content before considering the withdrawal complete; this workflow has no automated teardown or verified operator API for that operation.

Sites uploads only `apps/sites/build/` as `web-preview-sites` for inspection. This artifact has no automated deployment consumer and is not a complete runtime package. The planned preview uses a container image built from the pull request and runs in Kubernetes at a shared hostname supplied by deployment configuration. It is manually triggered and expires after six hours. The infrastructure owner supplies the cluster configuration; application deployment and the dev/prod service selector are not implemented. The PR comment reports that the preview is pending.

A newer publication cancels the older publication job for the same PR, including stale approval waits. Pending, upload-progress and CDN-completion comment writers share one serialized `web-previews-comments` queue using GitHub's `queue: max`; the pending update finishes before publication can start. Eligibility checks discard obsolete work, and paginated GitHub runs prevent an older build from replacing a newer comment. A verified-link marker bound to the source run and attempt prevents a late progress update from replacing its verified URL. Skipped metadata runs are discarded before PR-association fallback. Current-head cancelled or timed-out builds update the status and remove older preview links, but cannot publish. Publication eligibility rejects older meaningful same-head runs before approval and rechecks immediately before packaging and publishing; approval also requires the pending comment job to succeed. GitHub API reads and idempotent PATCH requests retry transient server and rate-limit failures at most three times, waiting at least 60 seconds for rate limits; comment-creation POSTs are not retried because the first request might already have created the comment. Source validation has a five-minute job timeout. PR association uses the PR number, base ref and head SHA; advancing the base tip alone does not invalidate a build of the same head.

### Local checks

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
