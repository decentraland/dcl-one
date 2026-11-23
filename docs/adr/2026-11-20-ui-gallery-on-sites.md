# ADR: Sites serves the runtime component gallery at `/ui-gallery/`

- Status: Accepted
- Date: 2026-11-20

## Context

The runtime component gallery (`library/ui/gallery`, [ADR](2026-11-20-ui-runtime-gallery.md)) could be reviewed only from a CI artifact or a local build. Its hosted mode (`vite build --mode hosted`) already builds a self-contained static gallery with relative URLs and no Figma reference, which works under any path prefix. The Sites image already carries another static build beside its client: the Explorer under `/play/`.

## Decision

- `infra/packages/ui-gallery.nix` builds the hosted gallery from the `library/ui` sources it compiles (the package's runtime sources, `apps/hud`, the gallery and `library/explorer-core`) with the package's own lockfile, pinned as a fixed-output pnpm dependency set. It carries no store references.
- `infra/packages/sites.nix` builds it (it is not an Explorer input, unlike `infra/packages/default.nix`) and links it into the client directory as `build/client/ui-gallery`, and fails if the Sites client has its own `ui-gallery` entry. The Sites server answers static files before its routes, so `/ui-gallery/` serves `index.html` and `/ui-gallery` redirects to it.
- The gateway sends `= /ui-gallery` and `^~ /ui-gallery/` to Sites; a Sites route file with that prefix fails the render.
- The gallery's HTML entries carry `<meta name="robots" content="noindex">`.
- The HUD canvas (`hud.html`) is a same-origin iframe on the origin `/play` serves, so the HUD code takes it for the Explorer's web shell. Its entry first replaces the document's storage (`gallery/src/hud/isolateStorage.ts`): Web Storage and cookies in memory, IndexedDB and Cache Storage refused, so the HUD's preferences, Web Storage and cookies written in the canvas stay in that page. It does not isolate everything: BroadcastChannel and same-origin access through `window.parent` remain available to the canvas. The HUD's own persistence is unchanged.
- The image build caches the compiled gallery as its own runtime entry (`ui-gallery` in `tooling/image-cache.py`), so a change to `apps/sites` or to the gallery alone misses only its own entry; the `library/ui` sources both compile miss both.
- An image is published only with a key-scan receipt for the gallery bytes it ships. The Shared UI library run puts its hosted build in the scanned `ui-review-<sha>` artifact as `ui-gallery/` (the two builds are byte-identical). Every images build that can publish (main pushes, tags, manual runs on main asked to publish) runs `tooling/ui-gallery-receipt.py`: it digests the image's `build/client/ui-gallery` (`gallery/scripts/hosted-digest.mjs` computes the same digest) and requires, for the head or, newest first, an ancestor that last changed a gallery input (`library/ui`, `library/explorer-core`, `infra/packages/ui-gallery.nix`, `flake.lock`), a run of the trusted `ui-library-key-scan.yml` (default branch, `workflow_run`, created between the start of the latest attempt of that commit's Shared UI library push run and an hour after it ended) whose key-scan job succeeded and whose log names the commit, the key check and the id of that run's `ui-review-<sha>` artifact; that artifact must hold the image's bytes. The newest such scan decides, and a pull request's scan of the same commit does not count. It waits while the newest candidate's run or scan is unfinished. It reads runs, jobs, logs and artifacts over the REST API with `actions: read`, and any failed, empty or malformed answer fails it. The "UI library key scan" check run is not read: any workflow with `checks: write` can post one, and one posted from a `workflow_run` lands in the triggering run's check suite, so its suite cannot name the scan workflow. A missing or failed scan, a missing artifact or a mismatch fails the build, so nothing is published. Main artifacts are kept 90 days for this. A hidden path fails the digest, since the artifact upload would drop it.
- `infra/packages/ui-gallery.nix` and `flake.lock` are in the Shared UI library scope (both workflows' pathspecs), so a commit that changes the Nix build gets its own scan.
- The Shared UI library workflow builds the Nix package (`ui-gallery-nix` job) whenever the library job runs, so a gallery change that imports a file outside the package's fileset fails on its pull request.

## Consequences

The receipt compares the scanned CI build and the Nix build byte for byte, so both build with Node 24 (setup-node in the library job, `nodejs-slim_24` in `infra/packages/ui-gallery.nix`; a test keeps the majors equal) and the same lockfile. A toolchain change on one side only makes main image builds fail at the receipt step, whose message names both places to reconcile.

The gallery on a deployment is the one of the Sites image it runs, so a gallery change reaches it only with a Sites deploy from main. A change to `library/ui/package.json`, `pnpm-lock.yaml` or `pnpm-workspace.yaml` updates the dependency hash in `infra/packages/ui-gallery.nix` in the same change; a pull request that changes one of them builds the images (`IMAGES_PACKAGING` in `tooling/bevy-ci.py`), so a stale hash fails there and not on main.
