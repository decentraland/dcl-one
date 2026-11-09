# ADR: The Shop's portable core in `library/shop`

- Status: Accepted
- Date: 2026-11-09

## Context

The web Shop's logic (model, prices, sale and payment rules, cart partitioning, contracts) lives
in `apps/ui3/src/shop`. The `/play` HUD (`apps/ui3/src/play`) imports it from there, which is
why the in-world Shop exists only on `/play`. The Explorer's own HUD
(`rendering/bevy-explorer/source/react-web`), which ships in the desktop app and the CDN web
Explorer, is a separate package and cannot import `apps/ui3`.

## Decision

Place the Shop logic that depends on no host in `library/shop`. Every Shop host imports it as
`@shop/*`: the web Shop and the `/play` HUD in `apps/ui3`, `apps/sites` and the Explorer HUD.

- **No host.** Its modules compile against ES alone, plus `console` and `TextEncoder`
  (`library/shop/tsconfig.json`). Configuration, network access, storage, identity, signers and
  wallets stay in the hosts, which pass them to the core through explicit adapters.
- **Allowed dependencies:** its modules import only the core, `@catalyst-api/*` and `viem`. No
  UI framework and no query library: query options, hooks, providers and components stay in each
  host.
- **No `package.json` or lockfile.** Each host resolves the core's packages to its own copy
  (`paths` in its `tsconfig.json`, `resolve.dedupe` in Vite or Vitest), at the versions the hosts
  already pin.
- **It moves in slices.** A slice holds only modules whose whole import closure meets these rules.
  Modules that still need the host move once the host is passed in, and that change is reviewed
  apart from the move.

### Layout

`library/shop` is used by several deliverables (Sites, the `/play` HUD, the Explorer HUD), as
the [layout ADR](2026-10-19-organizing-library-services-folders.md) requires of `library/`. It
is a TypeScript source library, like `library/catalyst-api`, so that ADR's crate rules do not
apply. It holds no UI: Shop components belong in `library/ui/apps/shop/`
([shared UI library ADR](2026-10-30-shared-ui-library.md)).

## Consequences

`library/shop/boundary.test.ts` enforces the rules, and `apps/ui3` runs the core's tests and
type-checks it under its own profile. A host that imports the core adds the alias and resolves
its packages. The Explorer HUD's build inputs include the shared sources, so changing the core
rebuilds the HUD. The web Shop keeps the rest of its logic in `apps/ui3/src/shop` until each part
can move without a host.
