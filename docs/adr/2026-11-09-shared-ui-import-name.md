# ADR: Shared UI import name

- Status: Accepted
- Date: 2026-11-09

## Context

The [shared UI library ADR](2026-10-30-shared-ui-library.md) put the shared
foundations and components in `library/ui`, imported as source, and left the
import name open. `apps/sites` and `apps/ui3` now consume both.

## Decision

The import name is `@dcl/one-ui`, the library's package name.
`import { theme } from '@dcl/one-ui'` resolves to `library/ui/index.ts`
(foundations); `import { Button, textClasses } from '@dcl/one-ui/components'`
to `library/ui/components/index.ts` (shared components and the class of each
text style); `@dcl/one-ui/<path>` to `library/ui/<path>`. Components are a
separate entry because the Explorer HUD's provenance (`tooling/play-artifact.py`,
`HUD_SHARED`) pins what `@dcl/one-ui` reaches to `index.ts` and `foundations/`.
Nothing is published or installed: each consumer resolves the names in
TypeScript (`paths`) and in its build (`resolve.alias`, plus `server.fs.allow`
for Vite's dev server) and bundles the source with its own single React and
React DOM. `apps/sites`, `apps/ui3` and the Explorer HUD
(`rendering/bevy-explorer/source/react-web`, foundations only) do.

`library/ui/foundations/` is the consumer entry for tokens and text styles.
`foundations.css` carries the tokens and the library's Inter faces
(`typography/inter.css`, variable Inter from Git LFS); the HUD loads it. The
web apps load the token sheets (`colors/colors.css`, `spacing/spacing.css`,
`layout/layout.css`) and declare Inter themselves: `apps/ui3/src/atoms/inter.css`
holds the Inter v20 faces decentraland.org's Shop loads, which the parity
harness measures against, declared once per document (a second copy of the
`@font-face` rules, or a variable face beside them, shifts Chromium's
rasterization by a sub-pixel). Readable token names alias the generated ones
and never redefine a value. The app serves the Inter woff2 files at `/fonts/`
(`apps/ui3/public/fonts`), so a page preloads them by a stable URL and a
non-bundled consumer loads the stylesheet unchanged.

## Consequences

- Build inputs and CI selection include `library/ui/index.ts`, `foundations/`
  and `components/`: the Nix Sites package source set lists them; the
  workflows that build or test the apps select on `library/**`.
- A consumer never copies a token value. A value without a token stays a
  literal with a comment naming the reference value, which
  `tooling/ui_standards.py --tokens` counts as drift.
- When promotion replaces `foundations/`, consumers keep importing
  `@dcl/one-ui`; the readable aliases are re-pointed, not the pages.
