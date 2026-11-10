# `library/shop`

The Shop's portable core ([ADR](../../docs/adr/2026-11-09-shop-portable-core.md)): the model (addresses, assets, chains, money, purchases, settlement and
its wire shapes), card prices, sale and rarity rules, the price slider, currency formatting,
payment rules and copy, cart partitioning, the spend guard, contract ABIs and the `/shop-api`
endpoint map. Every Shop host imports it as `@shop/*`: the web Shop and the `/play` HUD in
`apps/ui3`, and `apps/sites`. The Explorer React HUD is to follow.

## Rules

- TypeScript modules only: no components, scripts or styles, no React or TanStack.
- Its modules import only the core itself, `@catalyst-api/*` and `viem`. Hosts resolve `viem`
  to their own copy (`resolve.dedupe` in Vite, `paths` in `tsconfig.json`), since this folder
  has no `node_modules`.
- It reaches for no host. `tsconfig.json` compiles the modules against ES alone, plus the
  globals every host has (`globals.d.ts`: `console`, `TextEncoder`, `URL`, `URLSearchParams`,
  `AbortSignal` as a type only, and the `DOMException` constructor). So `window`, `document`,
  `fetch`, storage, `location`, `WebSocket`, `process` or `import.meta.env` do not compile.
  Configuration, identity, signers and wallets stay in the hosts and are passed in.
- `boundary.test.ts` checks the imports (static, side-effect, dynamic and `require`), that the
  compile profile stays ES only, and that every module loads in node with no DOM. Tests may also
  import `vitest` and `node:` builtins, and are not held to the ES-only profile.

## Tests

`apps/ui3` runs them as its `shop-core` Vitest project (`pnpm --dir apps/ui3 test`). Its
`typecheck` script checks them under its own `tsconfig.json` and the modules under this folder's.
