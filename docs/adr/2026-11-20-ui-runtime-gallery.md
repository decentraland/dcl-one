# ADR: Runtime component gallery in `library/ui/gallery`

- Status: Accepted; superseded in part by [2026-11-20](2026-11-20-ui-gallery-hud-showcase.md)
- Date: 2026-11-20

## Context

The generated Review (`library/ui/figma/review`) shows only the previews generated from the Figma capture. The hand-written components that `@dcl/one-ui` exports had no review surface. Every tracked non-test file under `library/ui/figma/` is a port input ([ADR](2026-11-10-port-verification-binds-port-inputs.md)), and `library/ui/package.json`, `pnpm-lock.yaml` and `tsconfig.json` are bound to the component generation record, so the gallery cannot live in, or be configured through, those files.

## Decision

Add `library/ui/gallery/`: a browser app that imports components from the package root (`@dcl/one-ui`) and loads `@dcl/one-ui/foundations.css`, as a consumer does. It keeps its own Vite config and `tsconfig.json`, its examples (never colocated in `components/`, which the runtime tsconfig compiles) and a build check. Nothing in the package imports it.

- A test enumerates the React components the package root exports and fails when one has no example and no exclusion with a reason.
- Examples render inside an iframe, so a chosen width is a real viewport for media queries.
- The build writes beside the generated Review (`figma/review/review-dist/gallery/`), so the existing Review artifact carries both routes and the existing key scans cover it. The gallery opens the generated Review as its second route without changing it.

This is the `library/ui` counterpart of the layout ADR's rules: the folder serves the shared UI deliverables and is owned with the rest of `library/ui`.

## Consequences

Adding a public component export requires a gallery example or a reasoned exclusion in the same change. The HUD shared-input manifest records `library/ui`'s directory names, so it is regenerated when the folder is added or renamed. The gallery does not cover the HUD design primitives in `library/ui/apps/hud` or the app compositions in `library/ui/apps`.
