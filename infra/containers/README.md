# Production images

The root flake builds Linux AMD64/ARM64 images; LiveKit, PostgreSQL and NATS
are external. Explorer images: pipeline.

| Output (local tag) | Command | State |
| --- | --- | --- |
| `image-apis` (`foundation-apis:nix`) | `catalyst serve`, `index`, `pulse`, `schema`; `cast-bot` | `/data`; PostgreSQL, blobs |
| `image-sites` (`foundation-sites:nix`) | `sites-server` | `/data` |
| `image-sites-polygon-credits` (`foundation-sites-polygon-credits:nix`) | `sites-server`, its Shop settling credits on Polygon through dcl-one's own CreditsManager | `/data` |
| `image-gateway` (`foundation-gateway:nix`) | `gateway` (8080) | `/tmp` |

- Run API, indexer, Pulse and migrations from one `apis` digest; domain crates are not separate services.
- The community chat message router, `catalyst-message-router`, is a separate binary that links libwebrtc (`cargo build -p catalyst-message-router --features livekit`); no image carries it ([message router](../../services/catalyst/docs/comms/comms-crate.md#message-router)).
- The APIs image has no SDK Play embed (`PLAY_EMBED_DIR` unset) and no `scene-build.wasm`.
- The APIs image carries `cast-bot` (`.#cast-bot`, `infra/packages/cast-bot.nix`), the LiveKit
  participant `serve` starts once per Cast presentation, and sets `CAST_BOT_BIN` to it, so
  `POST /cast-presenter-service/presentations` does not answer 503
  ([ADR](../../docs/adr/2026-11-17-cast-presenter-bot-process.md)). It is a package of its own:
  `catalyst` never links libwebrtc. Its build takes the Explorer's patched LiveKit SDK
  (`infra/packages/patched-source.nix`) and the prebuilt libwebrtc of the SDK's `WEBRTC_TAG`,
  fetched ahead by hash and passed as `LK_CUSTOM_WEBRTC`; Linux compiles the bridge with
  clang 21, as the Explorer's package does. The image smoke test runs it once.
  That libwebrtc links Chromium's FFmpeg statically (LGPL-2.1-or-later, configured without its
  GPL parts) and enables the H.264 and H.265 codecs, as the Explorer's own copy does. The image
  carries the notices at `share/doc/cast-bot/libwebrtc-LICENSE.md` in the package. FFmpeg's
  corresponding source is Chromium's `third_party/ffmpeg` at the revision the `DEPS` of WebRTC
  commit `a545de4` pins (the archive's tag, `webrtc-a545de4-prefixed-2`). `cast-bot` itself is
  built from this tree, so it can be relinked against a modified libwebrtc.
- Cast client composition: `CAST_PRESENTER_PUBLIC_URL=https://cast-presenter-service.<base domain>`
  on the APIs lets a client that composes the slides fetch the slide pictures (the Unity
  Explorer fetches them only from that host); unset or blank, clients show the bot's slide
  track ([ADR](../../docs/adr/2026-11-20-cast-client-composition.md)). The host needs a DNS
  record that points to the gateway and a certificate that covers it. Post-deploy check:
  `curl -s -o /dev/null -w '%{http_code}\n' https://cast-presenter-service.<base domain>/presentations/x/slides/x.png`
  prints `404`, the API's answer for an unknown slide; a DNS or TLS error or a `502` means that
  the host does not reach the API. Open gap: the slide picture route reads the session memory of
  one `serve` process, and the gateway spreads API requests over the replicas. Client
  composition needs one API replica, or `/cast-presenter-service` routed to one. With more
  replicas, most slide requests answer `404` and the Unity Explorer draws a black slide.
- Workflows are `<component>-<task>.yml`; Quay variables `QUAY_<COMPONENT>_REPOSITORY` hold full destinations without tags.
- Tags: `sha-<full-commit>[-<arch>]`, `main`, `v*` (not Bevy). Bevy SHA tags can change on replay: pin `repository@sha256:<digest>`.
- `image-sites-polygon-credits` is the Sites build with `VITE_SHOP_CHAIN_ID=137` and `VITE_SHOP_CREDITS_MANAGER` set to dcl-one's Polygon manager (`infra/containers/images.nix`, `shopSettlement` in `infra/packages/sites.nix`); the plain `image-sites` is unchanged. It is published to the Sites repository under its own suffix only (`sha-<full-commit>-polygon-credits`, `main-polygon-credits`), and names its settlement in `/shop-settlement.json` and the `org.decentraland.shop.*` labels.

## Sites Explorer build input

Sites bundles our Bevy web build under `/play/`; no other engine is used.

Sites also serves the runtime component gallery (`library/ui/gallery`, [README](../../library/ui/gallery/README.md)) under `/ui-gallery/`: `infra/packages/ui-gallery.nix` builds its hosted mode from `library/ui` with the package's pinned pnpm dependencies, and `infra/packages/sites.nix` links it into the client directory, so the server answers it as static files before its routes. The gateway sends `/ui-gallery` and `/ui-gallery/` to Sites. Its pages carry `noindex`. Only a Sites deploy from main publishes a change to it.

- `--production` imports only a fully successful main CI artifact and rejects `--from-build`.
- Dev and PR images may import once the web job uploads its artifact, or `--from-build` a local `make bevy-web`.
- A PR that changes no engine input (engine source, protos, HUD, build scripts) takes the newest main build that uploaded its web app and has not failed, validated against the commit that built it (`--trust-inputs` in the PR image build); a PR that does gets its own Bevy run and waits for that exact build. The bindings must match the tree either way.
- `PLAY_WEB_DIR` is a build input only. The imported files must carry no Nix store
  references: a WASM compiled inside a Nix shell embeds its toolchain's store paths, and
  imported from the store those paths would add the Rust closure (4 GiB) to the Sites
  image. The import refuses such an artifact; build it outside the store or with
  `--remap-path-prefix`.
- Image CI runs `--prepare-production RECEIPT`, then `--verify-production RECEIPT` before publishing.
- Rerun both PR workflows on the same merge SHA.
- Expired artifacts (main 90 days, PRs 7): `gh workflow run bevy-explorer-ci.yml --ref main`.

## Build and test

Linux, Nix 2.34+, Docker, Git LFS, `gh` authenticated:

```sh
git lfs pull && mkdir -p .cache/images
python3 tooling/fetch-play.py --production
export PLAY_WEB_DIR="$(python3 tooling/fetch-play.py --print-directory)"
nix build .#image-apis --out-link .cache/images/apis
nix build --impure 'git+file:.?lfs=1#image-sites' --out-link .cache/images/sites
docker load --input .cache/images/apis && docker load --input .cache/images/sites
python3 tooling/image-smoke.py
```

## Build and transfer reuse

- PRs build and smoke-test AMD64 only unless they change an
  architecture-sensitive input (the `arches` job in `apis-sites-images.yml` lists
  them); main, tag and manual runs build both architectures. Before the APIs image
  compiles, the build builds the Sites dependencies, so those failures report
  within minutes.
- Image caches separate Catalyst dependencies, Sites dependencies/toolchain and
  the compiled API (`catalyst` and `cast-bot`), Sites app and UI gallery (`ui-gallery`) runtime closures, keyed by exact Nix derivation
  identities and architecture. The Sites app (`sites-app`) is the compiled server and
  client with their production dependencies; the Explorer build and the docs media are
  assembled beside it as their own store paths, so neither changes its identity or its
  image layer. Its cache entry also holds the app of `image-sites-polygon-credits`
  (`sites-app-polygon-credits`), so an exact hit leaves neither Sites build to compile.
  The UI gallery is a separate entry: a Sites change does not rebuild it, and a change
  to the gallery's inputs does not recompile Sites.
  A compiled closure restored under its exact identity skips that
  package's dependency restore, import and toolchain build: nothing is left to compile.
  Container revision labels are still rebuilt for each commit. `cast-bot` has no
  dependency cache: when the APIs entry misses, it compiles whole beside `catalyst`.
- In Bevy CI the compile job (`apis-sites-compile.yml`) runs beside the web build, and
  the image build starts after both. The compile job looks for the three compiled closures
  under their exact keys without a download. When one is missing and this run can save it
  (the first attempt of a main push or of a PR), the job compiles the packages and saves
  the five cache groups, with the keys, the export checks and the save gate of the image
  build. The image build restores them and has nothing left to compile. Without a saved
  entry it compiles the packages itself, as tag and manual builds do. An entry holds the
  output of one derivation, so its content does not depend on which job saved it; the
  compile job saves before the image smoke tests.
- Successful main builds save trusted caches. PR builds save only their explicit
  PR namespace and merge-ref scope, the image build after its smoke tests. PR dependency caches
  may fall back from an exact main identity to the newest main entry of the group;
  a stale hit saves its replacement. Same-digest main hits need no duplicate save.
  A stale Catalyst hit reuses vendored sources; changed dependency derivations still
  compile from scratch. The fallback downloads the whole dependency archive.
  Main and tag builds require exact main identities and never select PR namespaces.
  The APIs, Sites app and UI gallery runtime caches require an exact identity for every event.
- A Sites cache miss rebuilds the pnpm dependency sets; the Node toolchain
  substitutes from cache.nixos.org on both architectures.
  Fully cold ARM64 main/tag dependency builds have not been timed. They retain the
  100-minute job limit; a timeout saves no replacement cache, so another run may
  also start cold and publication remains blocked until a successful build.
- Cache quota and idle eviction apply; dependency keys remain stable across
  source-only changes. Runtime caches exclude Cargo target trees. Hosted image
  jobs fetch Sites content and page media plus shared UI fonts from Git LFS and remove unused SDKs only when
  the runner starts with less than 50 GiB free. Smaller hosted disks still need
  that cleanup; larger runners skip it. Jobs log disk space before and after
  this check, memory before checkout, and disk space after cache import and image
  loads. After the Bevy artifact import, one `--impure` evaluation builds the three images;
  the APIs archive is written while Sites compiles, and it loads into Docker beside
  the Sites archives.
  Cache keys and export (`.`) and image builds (`git+file:.?lfs=1`) must resolve the
  same Catalyst, Sites app and UI gallery derivations. The Catalyst fileset takes from
  `library/` only what the backend build reads (the `dcl-*` crates, `vendor/rusty_enet`, the
  protocol sources and the OpenAPI documents), none of it LFS-tracked. Before each export,
  the job evaluates both references, using the image build's `--impure` mode for its
  ref. A mismatch or evaluation failure stops export before a second compilation.

- Closing a PR removes caches for its merge ref through a workflow that never
  checks out PR code. An hourly trusted sweep also removes uploads that finish
  after closure and, on main and open PR refs, retains only the newest smoke snapshot
  and Rust dependency cache per key prefix and platform. Image caches retain the
  two newest entries per group, architecture and ref. Older `v*` tags can miss their
  exact main cache after those generations are pruned and rebuild cold.
  Normal image builds upload only on their first attempt. Main must still match
  the built commit; PRs must still match the event's PR head SHA. Advancing main can
  regenerate a PR merge ref without suppressing a save for the unchanged PR head.
  Keys still describe the actual built derivations. A head change during
  export/upload can leave a late older entry;
  two generations reduce that race's impact but cannot guarantee an exact hit.
  Unrelated cache families are excluded from image retention.
- The Rust test and release-proof jobs cache dependency artifacts only
  (`Swatinem/rust-cache`): first attempts on main save, pull requests read, and the
  workspace crates rebuild on each run. The Explorer's native jobs key their
  `target` trees on the compiler, the tool environment, the Cargo inputs and the
  workspace tree (`tooling/bevy-target-cache.py`). The trees hold the workspace crates,
  so a job compiles only what a change reaches. The jobs fall back to the newest cache
  of the same dependencies, then of the same compiler. First attempts on main save an
  entry for each change of the workspace; a pull request of this repository saves one
  entry only when main has none for its dependencies. The smoke job keeps its full `target` snapshot.
  Cache pressure is logged by the sweep; quota and idle eviction still apply between sweeps.
- Imports use `--no-check-sigs`. GitHub cache scopes are shared across workflows;
  exact keys limit fallback but do not authenticate the writer. Publishing refs
  must run only trusted code with access to cache writes.

The [image cache policy ADR](../../docs/adr/2026-11-08-image-cache-policy.md)
describes cache scope, trust boundaries, stale-entry replacement and retention.

## Publication and runtime

- `QUAY_USERNAME`/`QUAY_TOKEN` robot secrets never enter builds or layers.
- PRs only build; `main` and `v*` publish; manual runs use the `publish` checkbox.
- Publication is not atomic: deploy APIs and Sites from one verified SHA or recorded digests.
- APIs and Sites set `COMMIT_HASH` to the build revision (`unversioned` outside git): APIs report it in `/about` and the status routes, Sites in `window.__DCL_PUBLIC__.commitHash`, which `/play`'s sign-in screens show as their build when the Vite build recorded none. A deployment's own `COMMIT_HASH` overrides it.
- APIs listen on 5210, Sites on 5212, as `1000:1000`. APIs need `DATABASE_URL`, posture, public URL, encryption key and integration secrets, and, behind the gateway, `HTTP_TRUSTED_PROXY_CIDRS` with the gateway's network ([gateway](../gateway/README.md)); Sites needs `CATALYST_URL`, `CATALYST_DATABASE_URL` and a session secret.
- Sites' `/download` takes two addresses from the deployment: `EXPLORER_INSTALLERS_URL` is the host of the Explorer launcher installers (unset, the page offers no installer, the home page's download call leads to `/download` and `/download_success` answers 404: the node serves none), and `DOWNLOAD_COUNTS_URL` is the JSON of the download counts per platform (unset or unreachable, the page shows no total and the home page shows "+400K").
- Behind the gateway, `/play` dials Pulse on the page's host. Pulse is UDP, which an HTTP-only front (a CDN proxy, a load balancer) cannot carry, so a deployment whose Pulse sits elsewhere sets `PUBLIC_PULSE_SERVER` (`host[:port]`, port 7743 when omitted) on Sites, which hands it to `/play` as `window.__DCL_PUBLIC__.pulseServer`. Where `/play` composes services from a base domain instead (a `decentraland.*` subdomain, or a `?baseDomain=` other than the page host), it ignores it. The engine writes the Pulse host into every link it syncs, so changing or removing `PUBLIC_PULSE_SERVER` later sends links that still carry the old host through the untrusted-link gate.
- Sites' blog (`/blog`) reads cms-server, the Contentful cache decentraland/sites reads: `BLOG_CMS_URL` is its full `<origin>/spaces/<space>/environments/<environment>` base. Unset, or with the CMS unreachable, the blog pages answer 503. TODO(cms-migration): point `BLOG_CMS_URL` at the in-tree catalyst CMS area once cms-server is migrated into dcl-one; today it is the external cms-server.
- Sites' sign-in (`/auth/login`) offers each option only when its public identifier is set, the same ones decentraland.org ships: `THIRDWEB_CLIENT_ID` (email codes), `MAGIC_API_KEY` (Magic's publishable key: Google, Apple, Discord, X) and `WALLETCONNECT_PROJECT_ID`. A different identifier signs the same account into a different wallet. Magic must allow the site's origin and `<origin>/auth/callback`. `FEATURE_FLAGS_ORIGIN` names the site whose `dapps-sign-in-primary-option` rollout lays it out (decentraland.org when unset). Locally, `tooling/dev.py` takes the three identifiers, and nothing else of the shell's, from the environment it is started in.
- `/play`'s PLAY AS GUEST uses a thirdweb guest wallet on `decentraland.*` hosts. Elsewhere the browser makes the guest's key and keeps only the sign-in it signs, so the guest gets a new wallet once that sign-in is within a day of expiring. On a deployment whose origin the thirdweb project allows, `/play?guestWallet=thirdweb` asks for the thirdweb guest instead; the browser keeps that guest's session, so later visits use it without the parameter. That choice stays with the browser until its site data is cleared, so an origin later dropped from the thirdweb project's allowlist needs that to go back to a local guest.
- Sites serves the email confirmation links under `/account` (`confirm-email-challenge`, `confirm-email`, `credits-email-confirmed`), so the node's `ACCOUNT_BASE_URL` is `<origin>/account`; a request on `account.<domain>` redirects there. `TURNSTILE_SITE_KEY` (Cloudflare Turnstile's public site key) shows the challenge and must pair with the node's `TURNSTILE_SECRET_KEY`; unset, the links confirm without one.
- Scene streaming keys (`/scene-stream-access`, the Admin Toolkit's "Live stream") are LiveKit Ingress RTMP endpoints: the APIs ask the external LiveKit's Ingress service for one and hand out the `url` and `stream_key` it answers. The deployment runs [LiveKit Ingress](https://github.com/livekit/ingress) beside its LiveKit server, both on one Redis: the server's config sets `redis` and `ingress.rtmp_base_url` (the public `rtmp://<host>:<port>/x` a streaming app pushes to), and the ingress config sets the same `api_key`/`api_secret` and `redis`, `ws_url` (the server's signaling URL), `rtmp_port` (public TCP) and `whip_port: -1` (the APIs create RTMP ingresses only). Without Redis, `/scene-stream-access` answers 500 (`ingress not connected (redis required)`); with Redis but no ingress process, it hands out keys that nothing listens for. The ingress refuses a stream with `server capacity exceeded` unless `cpu_cost.min_idle_ratio` of the machine's CPU (default 30%) stays idle after the stream's `rtmp_cpu_cost`. `tooling/dev.py` runs the same pair locally.
- Run `schema apply` (empty) or `schema migrate` as a job before the new API. Never reset an existing database.

Event poster upgrade: run `catalyst schema migrate` before deploying the API. Step `0050_explore_poster_avif` allows mirrored AVIF posters to be stored with their detected media type and preserves existing poster rows.

Sign-in identity upgrade: run `catalyst schema migrate` before deploying the API. Step `0051_explore_auth_identity_request_id` adds the nullable `auth_request_id` that binds a parked identity to its deep-link request; parked identities stay unbound. Sites binds only Creator Hub sign-ins, whose pickup must then send `x-identity-verifier`.

Curation committee upgrade: run `catalyst schema migrate` before deploying the API. Step `0052_shop_committee_index` admits the `committee` index in `shop.chain_cursor`. Builder's committee routes admit only `BUILDER_ADMIN_ADDRESSES` until `MARKETPLACE_COMMITTEE_INDEX_ENABLED` (on under the mirror posture) has walked and published the Polygon committee.

Scene admin upgrade: the release script
`schemas/releases/scene-admin-place-keys.sql` requires a maintenance window. Stop old API
replicas before `catalyst schema migrate`. Then run
`psql -v ON_ERROR_STOP=1 -f schemas/releases/scene-admin-place-keys.sql`
with the administrative role before starting the new binaries. It moves the scene admin grants
of active deployments from the scene entity to its place. An old replica reads and writes
grants by entity id: after the release it refuses every admin, and a grant it writes is lost.
`catalyst serve` refuses startup while a grant of an active deployment has the entity key.
Keep the pre-migration database backup: rollback must restore the matching backup.

Stream key upgrade: `catalyst schema migrate` creates five notification triggers on `worlds.world_scenes`, `worlds.worlds`, `worlds.world_permissions` and `worlds.world_permission_parcels`. Each `CREATE TRIGGER` takes a `SHARE ROW EXCLUSIVE` lock: writes to these four tables wait until the migrate transaction ends; reads continue.

Cast upgrade:

- Redeploy the gateway image with the APIs: the rule that sends the `/comms-gatekeeper/` and `/cast-presenter-service/` paths of the realm hosts (`worlds-content-server`, `realm-provider`, `realm-provider-ea`) to the APIs unprefixed is in the gateway image. Check: `curl -s -o /dev/null -w '%{http_code}\n' https://worlds-content-server.<base domain>/comms-gatekeeper/scene-admin` prints `400` (the gatekeeper refuses the unsigned request); `404` means that the gateway is the old one.
- The first start of the stream key listener (`comms-streaming-key-listener`) checks every active stream key and revokes each key whose scene was deployed again or removed since the key was made. Expect one burst of owner notices after the deploy.
- After the deploy, watch the rate of 5xx answers of `POST /livekit-webhook`: a Cast join guard that fails now answers 5xx, and LiveKit sends the event again.

World-admission upgrade: the release script
`schemas/releases/world-scene-ban-keys.sql` requires a maintenance window. Stop old API and
scene-host replicas before `catalyst schema migrate`; the object pass installs the durable queue.
Then run `psql -v ON_ERROR_STOP=1 -f schemas/releases/world-scene-ban-keys.sql`
with the administrative role before starting the new binaries.
Keep the pre-migration database backup: old binaries cannot read stable world ban
keys, so rollback must restore the matching backup. See `DATABASE.md` for the
retention and token-remint behavior.

### Pulse generation ownership

`catalyst pulse` requires `DATABASE_URL` and a deployment audience in
`ARCHIPELAGO_CONTROL_V4_AUDIENCE` or `PULSE_V4_AUDIENCE`, matching serve's audience.
Apply the schema before starting Pulse. Its root process holds a PostgreSQL session
lease and commits a fresh UUID/epoch binding before opening admissions; another Pulse
for that audience cannot acquire the lease concurrently. Database errors fail closed.
The local development launcher waits for the schema-ready API before starting Pulse.

Upgrade Pulse and every Gatekeeper writer together while admissions are stopped;
older binaries do not enforce generation guards and cannot safely share this deployment.
A database restore also requires stopped writers and closed admissions. Start a fresh
Pulse acquisition after restore; a restored generation row alone is never permission
to resume an old process. Existing issued credentials retain their finite validity.
Lease-loss detection cancels local source work; it does not synchronously retract bytes
already delivered or fence statistics publication through SQL on every message.

### Notification email settings

- `NOTIFICATIONS_BROADCAST_HORIZON_DAYS` now defaults to 0: producer broadcasts are kept and listed forever, as upstream. Set it to 90 to keep the previous hide-and-prune behaviour.
- Without `SENDGRID_API_KEY`, a sender, a template or `SENDGRID_API_URL`, notification emails stay pending (expiring after `NOTIFICATION_DELIVERY_MAX_AGE_SECS`) instead of ending skipped. `SENDGRID_EMAIL_FROM` and `SENDGRID_EMAIL_TEMPLATE_ID` are accepted as fallbacks.
- confirm-email runs Turnstile only while `TURNSTILE_SECRET_KEY` is set and the `dapps-turnstile-verification` flag is on (`FF_URL` or a local flag override); the node warns at start when the secret is set and the flag is off.
- set-email and confirm-email refuse the domains of `DISPOSABLE_EMAIL_DOMAINS_URL` and the `dapps-credits-blacklisted-domains` flag variant. Only `NODE_POSTURE=mirror` sets the URL; under island, journey or no posture it is empty and no disposable domain is refused unless the URL is set. A download with fewer than `DISPOSABLE_EMAIL_DOMAINS_MIN` (1000) domains keeps the previous list; lower it for a shorter list of your own.

### Pulse assignment upgrade

Selected room evidence changes both Pulse authority replies and stored assignment
stamps ([decision](../../docs/adr/2026-11-12-pulse-selected-room-evidence.md)).
New gatekeepers refuse old Pulse authority; old closed JSON readers refuse new
stamps. Use a coordinated maintenance window. This repository publishes images;
it does not implement or prove the ingress/orchestrator fence required below.

1. Fence native admission, signaling admission and every island-refresh mount,
   and drain existing signaling/native connections in the shared authority scope.
   `SOCIAL_WRITES_ENABLED=false` leaves admission open, and
   `FLEET_JOBS_ENABLED=false` alone is not this fence.
2. Preserve SQL assignments, revocation floors, cleanup dispatches and receipts.
   Reconcile unfinished cleanup while the matching authorities remain available
   behind the ingress fence, before replacing Pulse's in-memory authority;
   keep admission fenced while any residual credential or cleanup state is
   unresolved. Do not delete rows or infer selected identities from old strings.
3. Stop all incompatible Pulse and gatekeeper/`serve` processes, including
   refresh-serving API replicas. Replace them with one verified commit that supports
   selected identities or recorded matching image digests. Verify no incompatible process remains
   before allowing those roles to share live authority traffic.
4. With public admission still fenced, verify current selected Pulse evidence and
   residual SQL/SFU reconciliation. Old unproven SQL stamps must fail refresh;
   fresh authenticated admission must establish replacement evidence through the
   normal fenced path. Verify that every supported client re-admits after the
   empty 403 response instead of repeatedly refreshing the rejected stamp.
   Reopen only after the operator verifies these conditions.

These are release preconditions, not evidence that a deployment has met them.
After new stamps are written, rollback requires an image that supports selected
identities or a forward fix. Starting an older incompatible reader is unsupported; clearing
assignment or cleanup rows is not a rollback procedure.
