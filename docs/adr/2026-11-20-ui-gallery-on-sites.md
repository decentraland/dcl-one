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
- The HUD canvas (`hud.html`) is a same-origin iframe on the origin `/play` serves, so the HUD code takes it for the Explorer's web shell. Its entry first replaces the document's storage (`gallery/src/hud/isolateStorage.ts`): Web Storage and cookies in memory, IndexedDB and Cache Storage refused, so nothing done in the canvas reaches the storage real players' HUD reads. The HUD's own persistence is unchanged.
- The image build caches the compiled gallery as its own runtime entry (`ui-gallery` in `tooling/image-cache.py`), so a change to `apps/sites` or to the gallery alone misses only its own entry; the `library/ui` sources both compile miss both.
- The bytes Sites ships are the bytes the key scan reads. `library/ui/gallery/hosted-build.sha256` records the digest of the hosted build (`gallery/scripts/hosted-digest.mjs`). The Shared UI library run builds it, checks it against the record and puts it in the scanned `ui-review-<sha>` artifact as `ui-gallery/`; the Nix package checks its own build against the same record and fails otherwise. The two builds are byte-identical.
- The Shared UI library workflow builds the Nix package (`ui-gallery-nix` job) whenever the library job runs, so a gallery change that imports a file outside the package's fileset fails on its pull request.

## Consequences

The gallery on a deployment is the one of the Sites image it runs, so a gallery change reaches it only with a Sites deploy from main. A change to `library/ui/package.json`, `pnpm-lock.yaml` or `pnpm-workspace.yaml` updates the dependency hash in `infra/packages/ui-gallery.nix` in the same change; a pull request that changes one of them builds the images (`IMAGES_PACKAGING` in `tooling/bevy-ci.py`), so a stale hash fails there and not on main. A change to what the hosted gallery compiles updates `hosted-build.sha256` in the same change; both checks print the command that records it.
