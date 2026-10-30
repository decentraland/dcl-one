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

- All refs restore caches; only `main` saves. Tags restore from `main`, the default branch.
- `images-deps-node-all-<arch>-` holds complete Node sets; cold ARM64 runners may compile Node (METRICS.md).
- Delete obsolete `refs/pull/*` entries (`gh cache list`, `gh cache delete <id>`) once main has the replacement.
- Imports use `--no-check-sigs`: only trusted workflows may write caches a publishing branch consumes.

## Publication and runtime

- `QUAY_USERNAME`/`QUAY_TOKEN` robot secrets never enter builds or layers.
- PRs only build; `main` and `v*` publish; manual runs use the `publish` checkbox.
- Publication is not atomic: deploy APIs and Sites from one verified SHA or recorded digests.
- APIs listen on 5210, Sites on 5212, as `1000:1000`. APIs need `DATABASE_URL`, posture, public URL, encryption key and integration secrets; Sites needs `CATALYST_URL`, `CATALYST_DATABASE_URL` and a session secret.
- Run `schema apply` (empty) or `schema migrate` as a job before the new API. Never reset an existing database.
