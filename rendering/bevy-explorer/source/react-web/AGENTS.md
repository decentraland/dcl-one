# react-web — Agent & Contributor Guidelines

This app is the React DOM port of the Decentraland in-world HUD. UI here is built from
the design system: tokens plus the design primitives.

The HUD shared with the Sites `/play` host lives in two layers. The UI is
`library/ui/apps/hud` (`@hud/*`): the primitives (`@hud/design`), the tokens
(`@hud/styles/tokens.css`), the assets, the features and the session. The Explorer page's core is
`library/explorer-core` (`@explorer-core/*`), with no React: the wire protocol and bindings
(`@explorer-core/engine/*`), the drivers' contract, sign-in, shell messaging, analytics and server
reads. Write shared code there, not in `src/`; the core never imports the UI. It imports nothing from a host: a behaviour the hosts do
differently comes from `@hud-host` (`src/host.ts` here, implementing
`library/explorer-core/hostContract.ts`). These guidelines apply to it as to `src/`, and
`npm run typecheck` checks it. See `docs/ui-standards.md`, "Explorer HUD transition".

Design source of truth: the checked-in tokens and design primitives, together with the Explorer design system supplied through authorized project context. Keep private design-file identifiers outside this public tree.

> Before reviewing or merging anything in `react-web/`, read [`review.md`](./review.md).
> It is the always-on pre-merge source of truth: the test harness (tiers 1 / 1.5 / 2), per-domain
> expectations, the **§4 world-space (nametags / crosshair) manual checklist**, and the **§5 pre-merge
> review checklist** — every PR review runs through §5. This `AGENTS.md` defines *how to build*;
> `review.md` defines *how we prove it still works*. Keep `review.md` current when you add a domain or
> a rule (it goes stale — e.g. when engine-drawn behaviour like the nametags changes).

## 1. Always use the design system

- **Tokens.** Every color, radius, spacing, shadow, z-index, font size,
  and motion value comes from `library/ui/apps/hud/styles/tokens.css` (`var(--brand)`, `--panel`,
  `--text`, `--white-10`, `--green`, `--gold`, `--r-*`, `--fs-*`, `--dur-*`, …).
  Don't hardcode a brand/status hex or rgba, a radius, or a type size in a
  component. The only raw values allowed are layout primitives with no token
  (e.g. a one-off `gap: 4px`) — and even those should prefer a token when one fits.

- **Primitives.** Reusable interactive elements and surfaces use the
  components in `@hud/design` — `Button`, `IconButton`, `ControlButton`, `Panel`,
  `Icon`, not a bespoke `<button>` + CSS for something a primitive already
  covers (close/back/menu/toggle buttons, nav controls, cards, popovers).

- **If a primitive doesn't exist, CREATE it** in `library/ui/apps/hud/design/` — don't inline custom CSS
  for a pattern that will recur. A new primitive must:
  1. Live in `library/ui/apps/hud/design/<Name>.tsx` (+ `.module.css`), token-driven only.
  2. Be exported from `library/ui/apps/hud/design/index.ts`.
  3. Be added to `library/ui/apps/hud/design/Showcase.tsx` (viewable at `?showcase=1`).
  4. Support variants via props (size/variant/tone), not copy-paste.

## 2. When custom CSS is acceptable

Feature-specific **layout/composition** (e.g. chat bubbles, member rows, the emoji
grid, the loading overlay) may use local CSS Modules — but they MUST still be
**100% token-driven** and should compose design-system primitives for every button,
input affordance, and surface they contain. Bespoke CSS is for *arrangement*, never for
re-implementing a primitive.

## 3. Before adding a component, ask:

1. Does a `@hud/design` primitive already do this? → use it.
2. Is this a reusable control/surface with no primitive yet? → **create the primitive**,
   then use it.
3. Is this genuinely one-off feature layout? → CSS Module, token-driven, composing
   primitives.

## 4. DPI / HUD scaling (`--ui-scale`)

The HUD is scaled to the viewport (`useHudScale` sets
`--ui-scale` on `:root`, 1080-reference). Every floating panel / overlay MUST honor it
or it'll render too big on most displays:

- Full-screen pages scale via `MainMenuShell` (reference-canvas) — already handled.
- A **floating panel/popover** applies `transform: scale(var(--ui-scale)); transform-origin: …`
  itself (see `MainMenuShell.module.css`, sidebar).
- **Anything rendered through a React portal to `document.body`** escapes the HUD's
  transform, so it loses the scale AND fixed-positioning context. Re-apply
  `transform: scale(var(--ui-scale))` on the portaled element (and clamp to the viewport
  using its *measured* — i.e. already-scaled — size). `ProfileCard` is the reference.
- jsdom tests can't catch scaling/positioning (no layout). Verify floating UI **visually
  in `?mock=1`** (full HUD, no engine) before claiming it works.

## 5. Running the gates

The public tree omits the private gate specs and configurations. Follow
`review.md` for suite availability, and report unavailable gates as not run.

Agents that run gates in parallel must each use their own port set (`GATE_PORT`,
`GATE_BRIDGE_PORT`, `GATE_SERVICE_PORT`; `e2e/README.md` "Running gates side by
side"), never the defaults while another run may be up, and never a port another session's dev
server holds.

## 6. Checks

- `npm run typecheck` must pass.
- No new hardcoded brand/status colors, radii, or type sizes — grep your diff for raw
  `#` hex and `rgba(` and justify each (most should be a token).

## 7. The scene editor host

Create hosts the scene editor: its pages in `library/ui/apps/hud/features/editorHost`, its config,
source and host contract in `library/explorer-core/features/editorHost`. Use its checked-in pin, host contract and gates below. Resolve external editor development guidance through authorized task context; keep the target build self-contained. Here:

- The editor arrives through `editorHost/config.ts`. On `PINNED_EDITOR.hosts` (decentraland.org) it
  is one version checked against its `editor.js` hash, with its editor scene pinned and
  `?editor-tag` ignored; a new version is a PR with the editor repo's
  `node packages/web/scripts/pin.mjs <version> decentraland.org`. Elsewhere a released host
  follows `RELEASED_EDITOR`'s npm dist-tag (`latest`, or `next` with `?editor-tag=next`), loaded
  from unpkg with no hash (`releasedPin` in `source.ts`).
- A released host (`RELEASED_EDITOR.hosts`) offers Create only to an entry url with
  `editor-features=experimental`; `advanced` reaches the editor as `features.advanced`
  (`editorFeatures` in `editorHost/config.ts`). Loopback always offers it. A link can set them:
  they hide the editor, they are not a boundary.
- The contract is `window.__dclEditorHost` (`editorHost/host/host.ts`). The editor never holds a
  key: the host signs. Inside the web shell's app frame `localStorage` throws, so identity and
  signing go through `lib/shell`.
- The preview realm (`../deploy/web/PREVIEW_REALM.md`) and the scene server
  (`deploy/web/engine/headless.*`, `src/headless`) are engine-side; prove changes with the `editor`
  and `preview-realm` gates (`playwright.gate.config.ts`, `e2e/README.md`) and the editor host tests
  (`src/test/editorHost*.test.tsx`, `library/ui/apps/hud/test/hosted/editorHost*.test.tsx`).
- When the editor needs the engine to behave differently, change the engine here, in its own PR.

> Rationale: a single, enforced design system is what keeps the HUD consistent,
> themeable, and fast to evolve. Drifting into per-component custom CSS is the thing
> this file exists to prevent.
