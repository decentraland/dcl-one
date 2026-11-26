# ADR: Sites serves the avatar preview from the Explorer's web build

- Status: Accepted
- Date: 2026-11-25
- Amends: [2026-11-05 avatar preview renderer](2026-11-05-avatar-preview-renderer.md), its
  bullet on a separate preview workflow

## Context

The avatar preview renderer (`rendering/avatar-preview`) renders avatars, wearables and
emotes with the Explorer's crates behind the `wearable-preview` iframe contract. Hosts adopt
it by pointing their preview origin at a page that serves it. That page has to show what the
world shows, so its modules must come from the same Explorer crates as the in-world avatar;
it has to load fast on every Shop and creator page; and it has to need no credentials or
storage of its own.

Sites already serves the web Explorer at `/play/` from the play artifact: the build
`build-deploy-web` makes on each commit, which `tooling/play-artifact.py` fingerprints and
verifies and the Sites image links in (`infra/packages/play.nix`, `sites.nix`). Sites'
server (react-router-serve) sends `/assets/` as `public, max-age=31536000, immutable` and
every other client file with `max-age=0`, so each visit revalidates those.

## Decision

- The preview is part of the play artifact. `build-deploy-web` compiles both of its modules
  (WebGPU and WebGL2) from the same commit as the engine, packs them with a fingerprint of
  their inputs (the engine's inputs and the preview's crate) and caches them by that
  fingerprint, as it does the engine. A build that changes neither reuses them.
- `rendering/avatar-preview/tools/package.py` lays the build out for publication: the page
  (`index.html`) and, under `assets/`, every file it loads with a content hash in its name.
  Each module directory is named by the hash of its contents, so the wasm-bindgen glue keeps
  its relative imports. The page and `preview.js` are rewritten to the hashed names.
- Sites serves the page at `/avatar-preview/`, revalidated on every visit, and the hashed
  files at `/assets/avatar-preview/`, cached for good by the existing `/assets/` rule. The
  gateway routes `/avatar-preview` to Sites. A host switches by setting its preview origin
  to `<site>/avatar-preview`; the message protocol is the `wearable-preview` one.
- An optimized build fails when a module compresses (gzip -9) to more than 10 MB. Pull
  requests build the modules without wasm-opt, as they build the engine; `main` builds them
  optimized, checks the limit and publishes them with the rest of the artifact.
- The preview is a web lane input: a change to it or to the Explorer crates it links runs
  the web job, and a preview that fails to build fails that job. This replaces the separate
  preview workflow the 2026-11-05 decision named.

## Consequences

- The preview never drifts from the in-world avatar: both come from one build of one commit,
  and Sites ships them together or not at all.
- Returning visitors download a module only when its bytes changed; the page they revalidate
  is a few kilobytes.
- An Explorer crate change that breaks the preview fails the Explorer's web job, where the
  change is made.
- The production `wearable-preview` service, which the Marketplace and Builder also use, is
  unchanged. Pointing those hosts at this page is a later, separate decision.
