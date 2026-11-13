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

## Production APIs in use

dcl-one still reads these Decentraland-operated services. Each row is temporary until its exit criteria are met. Hosts are `*.decentraland.org` unless a row says otherwise, and the repo records no upstream owner for any of them.

How the host is chosen:
- **Catalyst:** `NODE_POSTURE=mirror` expands every row of `services/catalyst/config/src/posture.rs` (`ROWS`) to its Decentraland host. `island` and `journey` leave them empty. The repo does not record which posture each deployment runs, so a "mirror" row is production traffic only where a deployment runs that posture. The `mirror` dev profile sets the same values through `tooling/dev.py` (`runtime_env()`). The `offline` profile (used by CI's `dev-smoke.sh`) sets none.
- **Sites and ui3:** they reach the node through `CATALYST_URL` or the page origin, and the gateway serves the node's prefixes (`infra/gateway/routes.nix`, `root-aliases.nix`). Only the rows below leave that path.

Status values:
- **production:** deployed code calls the host by default;
- **mirror:** production under the mirror posture, and the dev `mirror` profile;
- **inactive by default:** production code that runs only when the named variable is set;
- **dev-only** and **CI-only:** what the names say.

| Entry | Where | Owner | Exit criteria |
| --- | --- | --- | --- |
| Content peers `peer`, `peer-eu1` (`/content`: `/snapshots`, `/pointer-changes`, `/entities/active`, `/contents/{hash}`). `SYNC_SOURCE` (`DCL_PEERS` in `posture.rs`), `READ_THROUGH_ENABLED`, `SYNC_FROM_DAO`. Status: mirror | `services/catalyst/content/src/sync/`, `content/src/read_through.rs` | `@decentraland/apis` | No target yet: this is how the local content area is filled |
| Chain RPC `rpc` (`/mainnet`, `/sepolia`, `/polygon`, `/amoy`, JSON-RPC). `ETH_RPC_URL`, `RPC_ENDPOINT_POLYGON`. When unset, defaults to `rpc.decentraland.org/<chain>?project=catalyst-content` (`config/src/config.rs`, `default_rpc_urls`) under mirror or no posture. Status: production | `content/src/sync/worker.rs`, `content/src/validator/`, `land-authz/src/`, `shop/src/{evm,mana,market,coupons}/`, `builder/src/handlers/publication.rs`, `src/index.rs` | `@decentraland/apis` | No target yet |
| RPC relay upstreams `rpc` (`RPC_UPSTREAM_{MAINNET,ETHEREUM,SEPOLIA,POLYGON,MATIC,AMOY}`); the OpenSea resolver's RPC legs reuse them. Status: mirror | `src/ops/rpc/routes.rs`, `explore/src/opensea.rs` | `@decentraland/apis` | No target yet |
| Shop checkout RPC `rpc` (`/polygon`, `/mainnet`, `/amoy`, `/sepolia`). `DEFAULT_RPC`, overridden by `window.__SHOP_CHAIN__.rpc` or `VITE_SHOP_RPC_*`; no deployment sets either. The market reader uses `rpc/amoy` only on Amoy. Status: production (browser) | `apps/ui3/src/shop/checkout/chain/config.ts`, `chain/rpc.ts`, `checkout/lib/authorizations.ts`; `apps/ui3/src/shop/market/lib/rpc.ts` | `@decentraland/web-applications` | Reads go through the node's relay (`${catalystBase()}/rpc/<network>`, as `apps/ui3/src/shop/land/evm.ts` already does), served by `services/catalyst/src/ops/rpc/` |
| Feature flags `feature-flags` (`/dapps.json`, Referer `FEATURE_FLAGS_ORIGIN`). Sites: `FEATURE_FLAGS_URL`, default `https://feature-flags.decentraland.org`. Shop browser fallback: `FLAGS_SERVICE`. Catalyst: `FF_URL`, `SHOP_FEATURE_FLAGS_URL` (mirror). Status: production | `apps/sites/src/data/lib/feature-flags.server.ts`, `auth/sign-in-flags.server.ts`; `library/shop/discovery/lib/rolloutFlags.ts`; `services/catalyst/social/src/communities/features.rs`, `shop/src/market/features.rs` | `@decentraland/web-applications`, `@decentraland/apis` | Sites and Shop read the node's `GET /feature-flags/<app>.json` (`services/catalyst/explore/src/api/feature_flags.rs`), which mirrors the document and applies local overrides |
| Explorer download counts `cdn-data` (`/public/current/explorer-alpha-downloads.json`). Hardcoded `COUNTS_URL`. Falls back to "+400K". Status: production | `apps/sites/src/data/lib/catalyst/landings/download-counts.server.ts` | `@decentraland/web-applications` | No target yet |
| Editor-scene realm `peer` (`/content`, `/lambdas`, `/bff`). The static realm JSON advertises them as `publicUrl`. Status: production (`/editor-scene/about`) | `apps/ui3/src/editor/editor-scene-realm.json`, `apps/sites/src/routes/routes/editor-scene.EditorScene.about.ts` | `@decentraland/web-applications` | No target yet. The node serves `/content` and `/lambdas` itself |
| Places `places` (`/api/places`, `/api/worlds`). `PLACES_UPSTREAM_URL`, `WORLDS_UPSTREAM_URL`. Status: mirror | `services/catalyst/explore/src/places/mirror.rs` | `@decentraland/apis` | No target yet |
| Events `events` (`/api/events`, `/api/events/{id}`). `EVENTS_UPSTREAM_URL`. Status: mirror | `explore/src/events/mirror.rs` | `@decentraland/apis` | `EVENTS_MIRROR_ADOPT` adopts the mirrored rows as local once the old service stops ([explore API parity ADR](adr/2026-11-05-explore-api-parity-contract.md)) |
| Event posters `events-assets-099ac00` (`/poster/{file}`, `/poster-vertical/{file}`). `EVENTS_POSTER_UPSTREAM_URL`. Status: mirror | `explore/src/api/posters.rs` | `@decentraland/apis` | No target yet |
| Worlds content server `worlds-content-server` (`/index`, `/world/{name}/{about,scenes,permissions}`, content). `WORLDS_CONTENT_MIRROR_SOURCE`. Read-through only in dev. Status: mirror | `services/catalyst/worlds/src/mirror.rs`, `content/src/read_through.rs` | `@decentraland/apis` | No target yet |
| Communities `social-api` (`/v1/communities`), thumbnails from `assets-cdn`. `COMMUNITIES_UPSTREAM_URL`, `COMMUNITIES_MIRROR_THUMBNAIL_URL`, and the `catalyst communities mirror` CLI defaults (`src/main.rs`). Off in dev. Status: mirror | `services/catalyst/social/src/communities/mirror.rs` | `@decentraland/apis` | No target yet |
| Denylists `asset-bundle-registry` (`/denylist`, every 120 s) and `config` (`/denylist.json`). `DENYLIST_URLS`, `EXPLORER_DENYLIST_SOURCE_URL`, `DENY_LIST_URL` (comms; not set in dev). Status: mirror | `content/src/denylist.rs`, `explore/src/api/denylist.rs`, `comms/src/archipelago/mod.rs` | `@decentraland/apis` | No target yet |
| Explorer lists, flags and map: `dcl-lists` (`/pois`, `/banned-names`), `feature-flags` (`/explorer.json`), `api` (`/v2/tiles`, `/v2/tiles/info`). `EXPLORER_LISTS_SOURCE_URL`, `EXPLORER_FEATURE_FLAGS_SOURCE_URL`, `EXPLORER_MAP_SOURCE_URL`. Status: mirror | `explore/src/api/lists.rs`, `explore/src/api/feature_flags.rs`, `explore/src/api/map/mod.rs` | `@decentraland/apis` | No target yet |
| Asset-bundle registry `asset-bundle-registry` (`/entities/{active,versions,status}`, `/worlds/{name}/manifest`, `/queues/status`). `EXPLORER_REGISTRY_SOURCE_URL`. Status: mirror | `explore/src/api/registry.rs` | `@decentraland/apis` | The local abgen runtime with native Registry state ([registry ADR](adr/2026-11-09-registry-local-catalyst.md)) |
| Asset-bundle CDN `ab-cdn` (`/{key}`). `ABGEN_UPSTREAM_AB_CDN`. Status: mirror | `explore/src/api/ab_cdn.rs` | `@decentraland/apis` | Empty: every object comes from this node's cache and abgen (`config/src/explore.rs`) |
| Marketplace trades `marketplace-api` (`/v1/trades`, `/v1/trades/{id}`). `TRADES_SYNC_UPSTREAM_URL`. Status: mirror | `services/catalyst/shop/src/market/trades.rs` | `@decentraland/apis` | No target yet |
| Subgraphs `subgraph` (`collections-ethereum-mainnet`, `collections-matic-mainnet`, `marketplace`, `land-manager`, `tpr-matic-mainnet`). `ETH_COLLECTIONS_SUBGRAPH_URL`, `MATIC_COLLECTIONS_SUBGRAPH_URL`, `MARKETPLACE_SUBGRAPH_URL`, `LAND_SUBGRAPH_URL`, `THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL` (not set in dev). Status: mirror | `chain-index/src/graph.rs`, `chain-index/src/land_graph.rs`, `builder/src/onchain.rs`, `shop/src/market/third_party.rs` | `@decentraland/apis` | LAND and permissions read the local land-authz and marketplace indexes once they are published (`config/src/shop.rs`); no target yet for collections and third-party registries |
| NFT worker `nfts` (`/wallets/{owner}/networks/{network}/nfts`). `NFT_WORKER_BASE_URL`; mirror refuses to start without it. Status: mirror | `shop/src/market/third_party.rs` | `@decentraland/apis` | No target yet. When it is unset, holdings are read over JSON-RPC |
| OpenSea fallback `opensea` (`/api/v2/chain/{chain}/contract/{contract}/nfts/{id}`). `OPENSEA_FALLBACK_URL`. Status: mirror | `explore/src/opensea.rs` | `@decentraland/apis` | No target yet |
| Profile images `profile-images` (`/entities/{entity}/{face,body}.png`). `PROFILE_IMAGES_ORIGIN_URL`, the last resort behind the local renderer. Status: mirror | `social/src/profile_images/origin.rs` | `@decentraland/apis` | No target yet |
| Media convert remote sources: mirrored place and event images on the peers' `/content/contents/...`. `MEDIA_CONVERT_REMOTE_SOURCES`. Status: mirror | `social/src/media/` | `@decentraland/apis` | No target yet |
| Foundation Builder `builder-api` (`/v1/collections`, `/v1/items`, `/v1/itemCurations`, `/v1/thirdParties`, publication writes). Catalyst verify: `BUILDER_FOUNDATION_URL` (mirror). Sites relay: `/api/builder-foundation/*` relays only when `NODE_POSTURE=mirror`, and answers 503 otherwise. Status: mirror | `services/catalyst/builder/src/linked_publication_review.rs`; `apps/sites/src/data/lib/catalyst/builder/foundation-relay.server.ts`, `foundation-fetch.ts`, `linked-providers.ts` | `@decentraland/apis`, `@decentraland/web-applications` | No target yet. The node's `/builder-api` does not replace Foundation curation |
| Builder storage `builder-api` (`/v1/storage/contents/{hash}`) and `builder-items` (`/{hash}`). `BUILDER_ASSET_PACKS_BUCKET_URL`, `BUILDER_CONTENT_BUCKET_URL`. Status: dev-only | `services/catalyst/builder/src/catalog.rs`, `tooling/dev.py` | `@decentraland/apis` | No target yet |
| Testnet upstreams: `contracts` (`/addresses.json`), `transactions-api.decentraland.zone` (`/v1/transactions`), and the `.zone` peer, marketplace and subgraphs of the `testnet` profile. Status: dev-only | `tooling/dev.py` (`testnet_env()`); `shop/src/evm/mod.rs`, `shop/src/evm/upstream.rs` | `@decentraland/infra`, `@decentraland/apis` | No target yet |
| Shop CMS images `cms-images`. Active only when `VITE_SHOP_CMS_URL`, `_SPACE_ID`, `_ENVIRONMENT` and `_ADMIN_ENTRY` (or `__SHOP_CMS__`) are all set; nothing sets them. Status: inactive by default | `apps/ui3/src/shop/discovery/lib/contentful.ts` | `@decentraland/web-applications` | No target yet |
| `/play` HUD services (realm provider, peer, places, events, map, marketplace, social, notifications and the rest). The HUD defaults to this deployment's gateway (`GATEWAY_PREFIXES`). It reaches `.org` or `.zone` only on a Decentraland origin, with the trusted `?baseDomain=`, or without `__DCL_PUBLIC__` (standalone). Status: inactive by default | `apps/ui3/src/play/lib/baseDomain.ts` | `@decentraland/web-applications` | Already served by the gateway prefixes; the exit is removing the `.org`/`.zone` fallback |
| Scene editor host: `worlds-content-server`, comms gatekeeper, storage and multiplayer on `.org` or `.zone`, and `creators-data` (`/v2`). Offered only on loopback, or on Decentraland hosts with `editor-features=experimental`. Status: dev-only on dcl-one deployments | `apps/ui3/src/play/features/editorHost/source.ts`, `editorHost/config.ts` | `@decentraland/web-applications` | No target yet. The node has `/worlds-content-server`, `/comms-gatekeeper` and `/world-storage`, but no document moves the editor host to them |
| `@dcl/sdk-commands`: `peer` and `peer.decentraland.zone` (`/lambdas/contracts/servers`, `/content/entities/active`, `POST /content/entities`), `quests`, `feature-flags.decentraland.zone`, `rpc/mainnet`. `DCL_CATALYST` (default `https://peer.decentraland.org`), others hardcoded. Status: production (creator CLI) | `library/js-sdk-toolchain/packages/@dcl/sdk-commands/src/` | `@decentraland/infra` | `apps/dcl-scene-cli` replaces the package; no per-endpoint target |
| Bevy Explorer E2E against `decentraland.zone` and `peer.decentraland.zone`. `vars.DCL_E2E_BASE_DOMAIN`, `vars.DCL_E2E_REALM`. Runs on the `ci:e2e` label, after main CI, nightly or by hand. Status: CI-only | `.github/workflows/bevy-explorer-e2e.yml`, `apps/bevy-explorer/e2e/driver/target.ts` | `@decentraland/client` | No target yet |
| Shop parity and trade journey harnesses: they record live traffic to the Shop's services and use `rpc` (`/mainnet`, `/amoy`, `/sepolia`). Run by hand; CI runs only their offline tests. Status: dev-only | `tooling/shop-parity/lib/services.mts`, `lib/session.mts`, `tooling/shop-trade-journey/lib/wallet.mts` | `@decentraland/web-applications` | No target yet |

Not listed: human links, canonical and sitemap URLs, URLs written into responses that the node does not fetch, mocks and fixtures, and third-party services (CoinGecko, publicnode, thirdweb, Segment and others).

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

PR builds execute with a read-only repository token and no publication credentials. `Web previews / Publish` independently uses code and the static key guard from its trusted default-branch workflow commit. It validates static artifacts, rejects retired comparison output and writes its own package metadata without lifecycle scripts; it never runs the PR gallery, source-owned browser checker or other PR source with publication credentials. Same-repository, open pull requests receive one updated status comment; forks build artifacts without publishing. Reviewer publication runs in the `web-preview` environment, which must require a human reviewer and disable administrator bypass. The source job validates that environment exists with those protections and the exact default-branch allowlist before scheduling publication approval. The same check runs after approval before handling artifacts and immediately before publishing. A failed preflight is reported as a publication-environment configuration failure with a link to the publication workflow. A pending status comment precedes the approval wait and replaces any older preview link; a configured maintainer may approve a build they authored. A failed or cancelled pending-comment job is reported explicitly and cannot release publication. A pending approval request also fails when source changes prevent the status from being posted. After approved publication, the comment reports upload in progress, an expected duration of about 6-10 minutes and a link to Actions for troubleshooting. A newer PR head or source-run attempt invalidates queued approval. A successful `cdn` / `upload-to-cdn` deployment for the package/version triggers the reader-free `Web previews / CDN completion` workflow, which posts a reviewer link only after verifying the matching source receipt, static HTML entry and a version-scoped hashed runtime asset.

Publication uses the shared organization secrets `GITLAB_CDN_DEPLOYER_TOKEN` and `GITLAB_CDN_DEPLOYER_URL`, retaining their existing Bevy consumers, and the repository secret `FIGMA_FILE_KEY` for the trusted artifact key scan. The publisher does not require `FIGMA_TOKEN` or environment-scoped Figma secrets; the source key scan remains environment-free. An explicit source-owned policy binding for the publisher workflow, scanning job and protected environment must land on the default branch before the publisher merges. The publication-environment decision remains an activation prerequisite. The trusted publisher, CDN completion workflow and dependency-free key guard must be installed on the default branch before publication can run; the gallery itself can remain in an open feature PR. The trusted scan uses `node library/ui/figma/key-guard.mjs --no-worktree --no-range --artifact .preview/input`. No Figma key is passed to the PR build. Activation remains pending verification of package registration in `definitions/src/static-site-rollouts.ts` and publication of `@decentraland/definitions@next`, initial npm package creation at placeholder version `0.0.0`, and npm trusted publishing for `@dcl-regenesislabs/dcl-one-ui-review` bound to this repository, `web-previews-publish.yml` and the `web-preview` environment. The GitLab upload pipeline must pin `https://registry.npmjs.org` and reject or ignore caller-selected alternate registries; otherwise a matching source receipt does not authenticate its uploader. Runtime environment checks cannot attest to secret storage, npm configuration or the GitLab registry policy. CDN completion rejects a version whose canonical base does not match its validated source receipt. The publisher requests package base version `0.1.<source_run_id>`; Oddish appends its timestamp and trusted publication-code commit suffix, keeping the CDN deployment tied to that default-branch workflow commit. Before activation, keep npm `latest` and `next` absent or below the requested base, because Oddish may otherwise override it. Previews publish only through custom `pr-N` tags without advancing `latest` or `next`. Retrying publication creates another immutable npm version. Npm registry polling is limited to 20 attempts at 10-second intervals. After the CDN upload success signal, transient receipt and runtime-resource failures share a 60-second retry window; source-identity mismatches and URLs escaping the version prefix fail immediately. Retrying that completion job checks the same version without republishing. The version commit suffix identifies trusted publication code; `preview-source.json` records the built PR head, source run and attempt. Re-runs replace the named build artifact, and an earlier attempt cannot reuse approval for a later attempt. GitHub artifacts expire after seven days; this does not delete immutable npm versions or CDN copies.

To withdraw a reviewer preview, first disable `WEB_PREVIEW_REVIEWER_ENABLED` to stop new publications. Record the affected package version, source receipt and complete preview URL, then request withdrawal from the npm/CDN operators and a cache purge for that version prefix. Removing a PR dist-tag or deprecating an npm version does not remove cached HTML or JavaScript. Confirm that the affected entry and bundle URLs no longer serve the content before considering the withdrawal complete; this workflow has no automated teardown or verified operator API for that operation.

Sites uploads only `apps/sites/build/` as `web-preview-sites` for inspection. This artifact has no automated deployment consumer and is not a complete runtime package. The planned preview uses a container image built from the pull request and runs in Kubernetes at a shared hostname supplied by deployment configuration. It is manually triggered and expires after six hours. The infrastructure owner supplies the cluster configuration; application deployment and the dev/prod service selector are not implemented. The PR comment reports that the preview is pending.

A newer publication cancels the older publication job for the same PR, including stale approval waits. Pending, upload-progress and CDN-completion comment writers share one serialized `web-previews-comments` queue using GitHub's `queue: max`; the pending update finishes before publication can start. Eligibility checks discard obsolete work, and paginated GitHub runs prevent an older build from replacing a newer comment. Only a late upload-progress or waiting-for-approval update may reuse a prior canonical CDN URL after its fetched source receipt matches the current PR head, run and attempt; it reconstructs the canonical comment body from trusted status data. Links from another run-derived package base are discarded before any receipt request. A transient receipt lookup failure leaves that call's existing comment unchanged and blocks dependent approval; a later ordinary diagnostic report can replace it with the explicit pending-status failure. Definite invalid or mismatched stored links, including malformed JSON or invalid Unicode receipts, are cleared. Comment markers and stored links alone are not publication authority. Skipped metadata runs are discarded before PR-association fallback. Current-head cancelled or timed-out builds update the status and remove older preview links, but cannot publish. Publication eligibility rejects older meaningful same-head runs before approval and rechecks immediately before packaging and publishing; approval also requires the pending comment job to succeed. GitHub API reads and idempotent PATCH requests retry transient failures at most three times within one shared 180-second CLI deadline. Each request reserves up to 30 seconds; rate-limit retries wait at least 60 seconds, with a longer delay only when the remaining shared budget can accommodate that delay and another request. Otherwise the command fails before sleeping or issuing the request. The 180-second deadline measures elapsed wallclock from CLI entry, including CDN verification and local work. Its guard enforces the remaining budget before GitHub API requests and retry waits, below the five-minute job timeout. Comment-creation POSTs are not retried because the first request might already have created the comment. Source validation has a five-minute job timeout. PR association uses the PR number, base ref and head SHA; advancing the base tip alone does not invalidate a build of the same head.

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
