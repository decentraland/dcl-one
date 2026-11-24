# `library/web-core`

The browser logic the web apps share, with no UI ([ADR](../../docs/adr/2026-11-18-shared-web-logic-library.md)). Sites and ui3 import it as `@web-core/*`; ui3 may import it, it never imports an app. The app-scoped UI in `library/ui/apps/` (the Sites flow map, and the in-world Shop session and wallet account) imports it too; `library/ui/components/` and `library/ui/foundations/` do not.

| Path | What |
| --- | --- |
| `validate/` | The zod `check` seam: `index.ts` re-exports the implementation the `dcl-validate-impl` alias names (`checked.ts`, or `unchecked.ts` in a performance build) with its failure counts and reporter. `vite.mjs` builds that alias for each host. |
| `auth/identity.ts` | Auth identity and chain types, the ephemeral key and login message, the stored shape and its storage key |
| `auth/signerCore.ts` | Signs a request with the ephemeral key and returns its auth-chain headers |
| `auth/thirdwebSchema.ts` | zod schemas of the thirdweb replies, and the auth result type |
| `auth/walletconnect.ts`, `auth/walletconnect-config.ts` | WalletConnect sign-in and its project id (page globals, Vite env or the host's `/internal/auth-config`) |
| `auth/eip1193.ts` | The EIP-1193 wallet provider type |
| `auth/wallet-events.ts` | Subscribes to a wallet's account and chain events and returns the unsubscribe |
| `auth/shopRequest.ts` | The Shop API mode, a transport over `fetch` and the request signer built from an identity |
| `profile/avatarColor.ts` | Display names, avatar colours and the synthetic avatar |
| `notifications/notificationsView.ts` | The notification row model, its view helpers and a loose parser |
| `site.ts` | The site's base URL, absolute URLs and host, for the browser and for a server render |
| `docs.ts` | The base and URLs of the documentation site |
| `newsletter.ts` | The site newsletter subscription request |
| `shop-chain-env.ts` | The Shop's chain selection from the build environment, with page overrides in development builds only |
| `place-view.ts` | Whether a place has a thumbnail of its own, and the viewer's relation to a place |
| `freshness/` | How old a cached copy is: its stamp, the older of two copies, a coarse duration and the per-section freshness model |
| `flow-map.ts` | The node, edge and section model of the flow map and its statistics |
| `creator-hub-host.ts` | The Creator Hub host contract: its paths, scope and the installed host |

## Rules

- No UI: no components, styles or React. Chrome contexts and components belong in `library/ui`.
- Its modules import only the library itself, the `dcl-validate-impl` alias, `zod`, `viem`, `@walletconnect/sign-client` and `qrcode-generator`; `boundary.test.ts` checks it. Hosts resolve those packages to their own copy (`paths` in `tsconfig.json`, `resolve.dedupe` in Vite or Vitest), since this folder has no `node_modules`.
- `tsconfig.json` compiles the modules against ES and the DOM plus `globals.d.ts`, so a module that relies on a global one app declares does not compile.
- One copy per page: the validation reporter and the WalletConnect client are module state, so every consumer imports this library rather than a copy.

## Tests

`apps/ui3` runs them as its `web-core` Vitest project (`pnpm --dir apps/ui3 test`); its `typecheck` script checks them under its own `tsconfig.json` and the modules under this folder's.
