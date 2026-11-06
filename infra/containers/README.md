# Production images

The root flake builds Linux AMD64/ARM64 images; LiveKit, PostgreSQL and NATS
are external. Explorer images: pipeline.

| Output (local tag) | Command | State |
| --- | --- | --- |
| `image-apis` (`foundation-apis:nix`) | `catalyst serve`, `index`, `pulse`, `schema` | `/data`; PostgreSQL, blobs |
| `image-sites` (`foundation-sites:nix`) | `sites-server` | `/data` |
| `image-gateway` (`foundation-gateway:nix`) | `gateway` (8080) | `/tmp` |

- Run API, indexer, Pulse and migrations from one `apis` digest; domain crates are not separate services.
- The APIs image has no SDK Play embed (`PLAY_EMBED_DIR` unset) and no `scene-build.wasm`.
- Workflows are `<component>-<task>.yml`; Quay variables `QUAY_<COMPONENT>_REPOSITORY` hold full destinations without tags.
- Tags: `sha-<full-commit>[-<arch>]`, `main`, `v*` (not Bevy). Bevy SHA tags can change on replay: pin `repository@sha256:<digest>`.

## Sites Explorer build input

Sites bundles our Bevy web build under `/play/`; no other engine is used.

- `--production` imports only a fully successful main CI artifact and rejects `--from-build`.
- Dev and PR images may import once the web job uploads its artifact, or `--from-build` a local `make bevy-web`.
- A PR that changes no engine input (engine source, protos, HUD, build scripts) takes the newest main build that uploaded its web app and has not failed, validated against the commit that built it (`--trust-inputs` in the PR image build); a PR that does gets its own Bevy run and waits for that exact build. HUD review and bindings must match the tree either way.
- `PLAY_WEB_DIR` is a build input only.
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
  compiles, the build checks the reviewed Sites HUD lock and builds the Sites
  dependencies, so those failures report within minutes.
- Image caches separate Catalyst dependencies, Sites dependencies/toolchain and
  the compiled API runtime closure, keyed by exact Nix derivation identities and
  architecture. Container revision labels are still rebuilt for each commit.
- Successful main builds save trusted caches. PR builds save only their explicit
  PR namespace and merge-ref scope after image smoke tests; they may restore an
  exact trusted-main match. Main and tag builds never select PR namespaces.
- The legacy `images-deps-node-all-<arch>-` cache is a read-only migration fallback
  when a split dependency cache is missing. Sites caches retain complete Node
  outputs; cold ARM64 runners may otherwise compile Node (METRICS.md).
- Cache quota and idle eviction apply; dependency keys remain stable across
  source-only changes. Runtime caches exclude Cargo target trees. Hosted image
  jobs remove all listed unused SDKs before the LFS checkout, retaining maximum
  cold-build headroom. Cleanup duration and free space are logged.
- Closing a PR removes caches for its merge ref through a workflow that never
  checks out PR code. An hourly trusted sweep also removes uploads that finish
  after closure and retains only the newest Development Rust snapshot per
  platform, purpose and ref; image and Explorer caches are not pruned by that rule.
- Multi-GB Rust PR snapshots require the `ci:cache` label and a same-repository
  Linux test job. Add the label before pushing the commit to cache; adding it
  alone does not start a new run. Main continues to refresh test and smoke snapshots. Cache
  pressure is logged by the sweep; quota and idle eviction still apply between sweeps.
- Imports use `--no-check-sigs`: only trusted workflows may write caches a publishing branch consumes.

## Publication and runtime

- `QUAY_USERNAME`/`QUAY_TOKEN` robot secrets never enter builds or layers.
- PRs only build; `main` and `v*` publish; manual runs use the `publish` checkbox.
- Publication is not atomic: deploy APIs and Sites from one verified SHA or recorded digests.
- APIs listen on 5210, Sites on 5212, as `1000:1000`. APIs need `DATABASE_URL`, posture, public URL, encryption key and integration secrets; Sites needs `CATALYST_URL`, `CATALYST_DATABASE_URL` and a session secret.
- Sites' sign-in (`/auth/login`) offers each option only when its public identifier is set, the same ones decentraland.org ships: `THIRDWEB_CLIENT_ID` (email codes), `MAGIC_API_KEY` (Magic's publishable key: Google, Apple, Discord, X) and `WALLETCONNECT_PROJECT_ID`. A different identifier signs the same account into a different wallet. Magic must allow the site's origin and `<origin>/auth/callback`. `FEATURE_FLAGS_ORIGIN` names the site whose `dapps-sign-in-primary-option` rollout lays it out (decentraland.org when unset). Locally, `tooling/dev.py` takes the three identifiers, and nothing else of the shell's, from the environment it is started in.
- Run `schema apply` (empty) or `schema migrate` as a job before the new API. Never reset an existing database.

World-admission upgrade: the release script
`schemas/releases/world-scene-ban-keys.sql` requires a maintenance window. Stop old API and
scene-host replicas before `catalyst schema migrate`; the object pass installs the durable queue.
Then run `psql -v ON_ERROR_STOP=1 -f schemas/releases/world-scene-ban-keys.sql`
with the administrative role before starting the new binaries.
Keep the pre-migration database backup: old binaries cannot read stable world ban
keys, so rollback must restore the matching backup. See `DATABASE.md` for the
retention and token-remint behavior.
