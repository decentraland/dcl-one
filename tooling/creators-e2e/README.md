# Creators e2e

Drives the creators pages of `apps/sites` (collections, the item editor, curation, the live preview) in a headless
Chromium, the way a creator or a curator uses them. Nothing leaves the machine: the wallet, the chain, the relayer,
builder-server, the marketplace and credits APIs, the flag service, the Catalyst and the wearable preview are fakes
in this folder, seeded per spec. It runs locally and is not part of CI.

```sh
pnpm install                                      # playwright-core, vitest and the fakes' dependencies, in this folder only
pnpm exec playwright-core install chromium        # unless CHROMIUM_BIN or a Nix Chromium is there
pnpm test                                         # every spec: starts the Sites dev server, then runs them one by one
pnpm test send-items                              # one spec
pnpm typecheck
```

Node 24+, with `apps/sites` installed (`pnpm install` there). The run starts `react-router dev` in `apps/sites` on
port 5283 (`E2E_PORT` moves it) and refuses to start if something already answers there, so a server from another
worktree is never tested by mistake. `CHROMIUM_BIN` picks the browser; otherwise the newest Chromium in the Nix store,
else playwright-core's own download.

## How it works

- `helpers/app.ts` opens a page on the dev server with a restored session for a deterministic test wallet
  (`signedOut: true` for a visitor) and answers every request through `helpers/route.ts`: the builder, the
  marketplace, the relay, credits, content and lambdas by their path on the node serving the page, everything else
  by the host the creators config names for the development environment.
- builder-server (`helpers/builderServer.ts`) and the marketplace (`helpers/market.ts`) are in-memory stores seeded
  from the `fixtures.ts` factories. Writes change them, so a spec asserts what the page shows after a save.
- One fake chain (`helpers/rpc.ts`) backs the read node and the wallet. Signatures are real (the test key, which holds
  nothing anywhere), every transaction settles at once and lands in `app.transactions`, decoded, and the fake
  builder-server applies what an indexer would. A wallet off Polygon goes through the fake relayer.
- The wearable preview iframe is a page that speaks its postMessage protocol (`helpers/preview.ts`).
- Telemetry is read off the requests the page sends: `app.waitForEvent('bd_items_added', { collection_id })`.
- Flags are off unless a spec passes `flags`, keyed `${application}-${flag}`; the site's `/internal/feature-flags`
  answers with them.
- Model fixtures are in `fixtures/files` (`helpers/files.ts` says which pass the rule book); zips are built at run time.

A request the pages make and the fakes do not answer is logged as `[e2e] unmocked …`: add it to the router or a fake,
never let it reach the network.
