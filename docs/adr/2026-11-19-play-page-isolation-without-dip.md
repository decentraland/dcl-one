# ADR: Page isolation for /play in browsers without Document-Isolation-Policy

- Status: Proposed
- Date: 2026-11-19

## Context

The web Explorer needs `SharedArrayBuffer`, so the app document (`app.html`: HUD and
engine) must be cross-origin isolated. The shell page (`index.html`, Sites' `/play`)
embeds the app in a frame and keeps the sign-in key. The app asks the shell for the
public identity and for signatures and never reads the key.

In Chrome 137 and later, `app.html` isolates itself with
`Document-Isolation-Policy: isolate-and-credentialless`. The browser then runs it in a
process of its own, and the shell stays unisolated in another process. Firefox and
Safari do not support that header. There the app is not isolated, and the engine stops
before it starts its workers. Firefox can isolate only a whole page, with
`Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy`.

## Decision

- **Capability, not user agent:** the HUD gate refuses a browser without
  `navigator.gpu`. It does not read the user agent.
- **The app reports missing isolation:** when the app is in the shell and is not
  cross-origin isolated, it asks the shell for page isolation (`isolation.page`).
- **The shell chooses page isolation once:** it keeps the choice in `localStorage`
  (`dcl-shell-isolation`), registers the service worker as
  `service_worker.js?isolation=page` and reloads once when that worker is active
  (`library/explorer-core/lib/isolation.ts`).
- **The service worker isolates whole pages:** with `?isolation=page` it gives the shell,
  the app, the scene server's frame and same-origin workers COOP `same-origin` and COEP
  `credentialless`. Without the parameter it keeps the policy above.
- **Chrome does not change:** the app is isolated in its frame, so it never asks, and the
  worker keeps its URL without the parameter.

## Rejected options

- **A user-agent check:** it gates browsers by name, not by the capability the engine
  needs, and it cannot follow a browser that adds the header later.
- **COOP and COEP on the shell for every browser:** Chrome would lose the separate process
  for the app.
- **Run the app top-level, without the shell:** the app would then hold the sign-in key
  in its own document.

## Consequences

- Open decision: with page isolation, the shell and the app share one process. The
  sign-in key is then in the same process as scene code that has `SharedArrayBuffer`.
  This is the risk that the Chrome design removes.
- The first visit in such a browser loads the app once without isolation, then reloads
  once more, through the page-isolation worker. Later visits load once.
- The choice stays in `localStorage`. If such a browser adds the header, it keeps page
  isolation until the site data is cleared.
- Firefox shows its persistent-storage prompt when the engine asks for persistent storage.
