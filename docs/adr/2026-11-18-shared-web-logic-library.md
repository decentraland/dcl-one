# ADR: Shared web logic in `library/web-core`

- Status: Accepted
- Date: 2026-11-18

## Context

`apps/sites` imports browser logic that lives in `apps/ui3`: the zod validation seam, the auth identity and request signer, the thirdweb reply schemas, WalletConnect sign-in, the profile display helpers and the notification view model. ui3's own Shop, Creator Hub and `/play` code imports the same modules, and runs on Sites routes. Sites must not import `apps/ui3`, and a copy per app would leave two validation reporters, two failure counts and two signers on one page.

## Decision

Put the non-UI browser logic the web apps share in `library/web-core`, imported as `@web-core/*`. Sites and ui3 import it; it imports neither.

- **Name.** `web-core` is the web apps' counterpart of `explorer-core`, the Explorer page's core both HUD hosts share: the logic under `apps/` web hosts, with no UI.
- **No UI.** No components, styles or React. React contexts and components the web chrome shares belong in `library/ui` ([shared UI library ADR](2026-10-30-shared-ui-library.md)); Shop rules belong in `library/shop` ([portable core ADR](2026-11-09-shop-portable-core.md)).
- **Allowed imports.** Its modules import only the library itself, the `dcl-validate-impl` alias, `zod`, `viem`, `@walletconnect/sign-client` and `qrcode-generator`. `library/web-core/boundary.test.ts` checks every static, side-effect and dynamic import.
- **Its own compile profile.** `library/web-core/tsconfig.json` compiles the modules against ES and the DOM plus `globals.d.ts` (the page's public configuration and Vite's env, as narrow as their use), so a module that relies on a global one app declares does not compile.
- **No `package.json` or lockfile.** Each host resolves the library's packages to its own copy (`paths` in its `tsconfig.json`, `resolve.dedupe` in Vite or Vitest), at the versions the hosts already pin.
- **The validation seam.** `validate/index.ts` re-exports the implementation the `dcl-validate-impl` alias names. `validate/vite.mjs` builds that alias: Sites takes the checking implementation; ui3 takes the one its build mode selects and adds its own performance-build stubs for ui3 schema modules.
- **One instance per page.** The validation reporter and failure counts and the WalletConnect client are module state; every consumer imports this library and none keeps a copy.

### Layout

`library/web-core` is used by several deliverables (Sites' landing, discovery and auth pages, and the Shop, Creator Hub and `/play` code ui3 renders), as the [layout ADR](2026-10-19-organizing-library-services-folders.md) requires of `library/`. It is a TypeScript source library like `library/shop` and `library/catalyst-api`, so that ADR's crate rules do not apply.

## Consequences

- `apps/ui3` runs the library's tests as its `web-core` Vitest project and its `typecheck` runs the library's profile; its `tsconfig.json` type-checks the tests.
- The Nix Sites package source set includes `library/web-core`; the workflows that build or test the apps already select on `library/**`.
- A module moves here only when its whole import closure meets these rules. A module that needs app UI or app state stays in its app until that dependency is passed in.
