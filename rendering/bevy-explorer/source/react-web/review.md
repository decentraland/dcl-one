# review.md — how we keep the HUD working

> **Suite availability:** this public tree includes the unit-test sources but omits
> the visual/engine gate specs, screenshot baselines and Playwright gate/visual
> configs described below. Those procedures apply only in an authorized test
> workspace that supplies them. Inspect the files at the target revision before
> running a tier; a package script alone does not prove its suite is present.
> Report unavailable tiers as **not run** and keep their evidence in the task report.

The single place to look before merging anything in `react-web/`. It defines the **test harness**
(what proves "everything still works"), the **per-domain expectations**, and the **pre-merge
checklist**. Keep it current: when you add a domain or a rule, add it here.

---

## 0. TL;DR — run this before you push

```bash
# from react-web/ — Node 24 (`.nvmrc`); on Node 20 vitest reports "no tests" and the SDK build fails
pnpm run typecheck          # tsc, the app and the shared HUD UI (library/ui/apps/hud)
pnpm test                   # tier 1 — the unit tests present at this revision, the shared HUD UI's included
# With the authorized visual suite and configuration present:
pnpm run test:visual        # tier 1.5 — visual regression (headless, no engine)
(cd bridge-scene && pnpm run build && pnpm run lint)   # the SDK's type check + eslint (must be 0 errors)
```

The bridge scene is type-checked by `sdk-commands build`, not bare `tsc` — its tsconfig trips
TypeScript's deprecation errors under the installed compiler.

`test:visual` reuses whatever is already serving `:5173` (and `pnpm run dev` reuses `:8100`), so in a
worktree make sure those aren't another checkout's servers, or run your own on other ports
(`E2E_URL=http://localhost:5174`).

If `test:visual` reports diffs, open the Playwright HTML report (`pnpm exec playwright show-report`) and
look at the **expected vs actual vs diff** images. Either it's a real regression (fix it) or an
intended change (refresh the baseline — see §2).

The real engine + the world-space UI it draws (3D nametags, crosshair) can't be screenshotted
deterministically — they have their own tiers (§3, §4).

---

## 1. The testing tiers (why three)

Connectivity feedback is engine-owned. `library/ui/apps/hud/test/hosted/connectivity.test.tsx` checks the shared
SystemApi stream/retry bridge, subscription cleanup, stable unchanged snapshots, one Connection Lost alert dialog,
pending Retry/Exit controls through bridge acknowledgement, reply timeout and session cleanup,
failed-retry recovery controls, automatic dismissal,
and scoped access messages. Runtime verification reads `/connectivity_state`, uses
`/connectivity_retry`, and locates `alertdialog` named `Connection Lost`, buttons `Retry` and
`Exit Application`, and status `Multiplayer status`. The engine's Connectivity debug section exposes the
same safe snapshot and Retry action. Preview, held-world and terminal access failures must
never become an offline alert. A reachable network with a failed or disconnected room shows its
room status. An initial `connecting` room stays quiet during ordinary scene crossings.

| Tier | Command | What it proves | Engine? | Deterministic? |
|------|---------|----------------|---------|----------------|
| **1 — contract** | `pnpm test` | Every page→scene API call posts the exact wire message, and every scene→page message updates state. Component clicks fire the right action. | no (FakeDriver) | yes |
| **1.5 — visual** | `pnpm run test:visual` | Every DOM **domain renders correctly** — layout, colour, tokens, spacing — diffed against a committed baseline PNG. | no (`?mock=1`) | yes |
| **2 — engine** | `pnpm run test:e2e` | The guest-reachable calls actually **round-trip through the real bevy engine + bridge scene**. | yes (headed, WebGPU) | partly |

Tier 1 is the contract net (every call, including ones a guest can't reach). Tier 1.5 is the
*appearance* net (the thing humans notice). Tier 2 is the *it-actually-talks-to-the-engine* net.
They're complementary; none replaces another.

> **Why visual regression over an "agent drives Chrome and looks" approach?** A committed-baseline
> Playwright suite is repeatable, runs in CI, pins the exact pixels, and needs no human/agent in the
> loop to pass. The agent-driven flow is great for *exploring* a new bug, but it can't be a
> regression gate. So: Playwright for the gate, agent checklist (§4) only for what can't be mocked.

---

## 2. Tier 1.5 — the visual harness (`e2e/visual.spec.ts`)

Renders the HUD in **mock mode** (`?mock=1`: fake bridge, fixed data, no engine/GPU) and screenshots
each domain. Config: `playwright.visual.config.ts` (headless, 1600×900, `maxDiffPixelRatio: 0.01`).

**Determinism is engineered, not hoped for** (`prepare()` in the spec):
- **Frozen clock** (`page.clock.setFixedTime`) → relative times ("2h ago", "Yesterday") are stable.
- **External images stubbed** → every non-localhost avatar/thumbnail becomes a 1×1 transparent PNG,
  so the network can't make a screenshot flake (layout is preserved; only the bitmap is blanked).
- **Animations disabled** at capture time; **fixed viewport + `deviceScaleFactor: 1`**.

**Domains covered** (one baseline each, in `e2e/visual.spec.ts-snapshots/`):
`showcase` · `login-fresh` · `lobby` · `mobile-gate` · `browser-gate` · `gpu-gate` ·
`engine-error` · `realm-error` · `world-hud` · `profile-card` · `passport` · `hover-tooltips` ·
`permission-dialog` · `permissions-popover` · `world-password-dialog` · `community-modal` · `community-create-modal` ·
`panel-friends` · `panel-settings` · `panel-settings-keybindings` · `panel-profile` ·
`panel-notifications` · `panel-emote-wheel` · `panel-communities` · `panel-map` ·
`minimap-settings` · `backpack-wearables` · `backpack-emotes` · `passport-edit` · `name-edit`.

Not yet baselined: the Events page (`library/ui/apps/hud/features/events`: the calendar, a day's
grid, the event detail). Mock mode serves it a week of events around now
(`library/explorer-core/engine/mockEvents.ts`; `?mock=1&mockEvents=error` fails every read), so a
baseline needs the frozen clock. Its contract is tier-1, in `library/ui/apps/hud/test/hosted`:
`events.test.tsx` (the reads sent, the loading, error, empty and loaded states, the day
navigation, the reminder, the detail, share and jump), `eventsStore.test.ts` and
`eventFormat.test.ts`; the bridge's requests are in `src/test/bridgeEvents.test.ts`.

Not yet baselined: the integrated-GPU guide (`features/gate/IntegratedGpuGuide.tsx`, opened over the
lobby or world when the pre-boot probe sees an Intel adapter on Windows; `?mock=1&gpuGuide=1` forces
it). Its contract is tier-1: `library/ui/apps/hud/test/integratedGpuGuide.test.tsx`, `library/explorer-core/lib/integratedGpu.test.ts`.

**Updating baselines** (only when the change is intentional):
```bash
pnpm run test:visual:update      # regenerates the PNGs
git add e2e/visual.spec.ts-snapshots && git diff --cached --stat   # then EYEBALL the new PNGs
```
Never blind-update. A baseline refresh in a PR must be reviewed image-by-image.

**Caveats / known limits**
- **Baselines are platform-specific** (`*-chromium-darwin.png`). Font hinting differs across OSes, so
  generate/verify on the same platform CI uses (or add a Linux baseline set when CI is wired).
- **Mock data is the ceiling.** A panel can only be as rich as `library/explorer-core/engine/mockBridge.ts` makes it.
  - Enrich mock `getEmotes` with an OWNED collection (some with `slot`, varied `rarity`/`count`) so
    the new owned-grid + assign-to-slot are exercised, not just the 10 base emotes.
- **Captures are viewport-sized, hover-free.** A scrolling panel is pinned only above the fold: the
  `passport` baseline shows About Me + the Equipped Wearables grid; Equipped Emotes, Badges, the
  About → Equipped → Badges order and the SHOP-link rule are tier-1 (`profile.passport.test.tsx`).

---

## 3. Tier 2 — real engine (`e2e/engine.spec.ts` + `e2e/engine-clicks.spec.ts`)

See `e2e/README.md`. Needs a real GPU + a local engine build (`../deploy/web`). Run before a release
or when touching the bridge protocol.

- **`engine.spec.ts`** — every panel *opens* and *fetches* (the call crosses the bridge).
- **`engine-clicks.spec.ts`** — the controls *inside* panels do something the **engine** sees,
  verified **through the bevy console API** where the engine exposes state, bridge-spy otherwise:

  | Click | Verified by |
  |-------|-------------|
  | move / teleport | `/player_position` lands at the exact coords (`helpers.position`) |
  | equip a wearable | `/get_user_data` profile **`vN` bumps** (`helpers.profileVersion`) + `equip` on the bridge |
  | assign an emote to a slot | `equipEmote` on the bridge |
  | play a wheel emote | `triggerEmote` on the bridge |
  | change a setting (slider) | `setSetting` on the bridge |
  | (sanity) profile + roster reachable | `/get_user_data`, `/connected_players` |

  Console helpers live in `e2e/helpers.ts` (`position`, `getUserData`, `profileVersion`,
  `connectedPlayers`). Available bevy commands: `move_player_to`, `walk_player_to`, `teleport`,
  `player_position`, `get_user_data`, `connected_players`, `live_scenes`, `list_portables`, `emote`,
  plus `/login_*`, `/logout`, `/chat`.

**Still tier-1-only** (a fresh guest can't reach them, so they can't round-trip here): accepting a
friend request, leaving a community, marking notifications read, community chat (the tabs come
from the communities the account is a member of; `library/ui/apps/hud/test/hosted/communityChat*.test.tsx`, and
`apps/bevy-explorer/e2e/specs/community-chat.test.ts` with a member account), creating, editing
and deleting a community (it needs an account with a claimed NAME), linking places to it (an account
that owns a LAND or a World with a deployed scene; `library/ui/apps/hud/test/hosted/communities.clicks.test.tsx` covers the
form, the places picker, the card's ADD PLACE and place delete, the content-check screens, the picture checks, the card's Edit and Delete, the member row and menu, and the browse grid's search, paging and card buttons; `library/ui/apps/hud/test/hosted/communities.invites.test.tsx` covers the Invites & Requests section and its counter, accepting and rejecting invites, the card's Banned, Join Requests and Invited sections, the Invite to Community submenu and the notifications that open a card; `library/ui/apps/hud/test/hosted/communities.announcements.test.tsx` covers the Announcements tab's paging, posting, deleting, liking, marks and toasts, and `src/test/bridgeCommunities.test.ts` the bridge's posts calls and the service's official mark; posting needs an owner or moderator account, and the official mark an author in the explorer feature flags' `alfa-official-wallets` list on a dcl-one node; acting on a member needs a second account in a community the first owns or moderates, an invite or a join request needs a second account too, and finding a community past the first browse page needs a service with more than 20) — these need seeded
data and are covered deterministically in tier 1. **TODO to deepen tier 2** (would need engine query commands or
fixtures): map *UI* teleport by clicking a parcel, a `/mic_state` query to confirm the mic toggle,
logout via the profile chip.

---

## 4. World-space UI — agent / manual checklist (can't be mocked)

The 3D nametags, the pointer-lock crosshair, and projected proximity tips are drawn **in the engine**
(the nametags by `crates/avatar/src/nametag`, the rest by the bridge scene), not the DOM, so `?mock=1`
can't show them and screenshots can't be deterministic. Verify these by hand (or have an agent drive
the Chrome extension against a live world) after any change to `crates/avatar/src/nametag` or
`bridge-scene/src/domains/{pointer,proximity,avatarPointer}.*`. `/nametag_state` reports every shown
tag (name, badges, distance, opacity, scale) and `/nametag_bubble` shows a test bubble:

- [ ] **Nametags** appear above every avatar's head, follow them while walking, and **face the camera**.
- [ ] Name **colour** = a claimed name's custom colour if set, else the hue hashed from the name (the
      colour the HUD shows for it); **claimed** names show the verified seal, **unclaimed** show `#abcd`
      tight to the name, dimmed. The name stays on one line.
      A name written with its own tag (`Ada#ffff`) still shows the address' tag, and a guest
      shows no seal whatever its profile claims.
- [ ] A name colour picked on the passport reaches the tag once the passport closes, on your own
      avatar and, for other players, on theirs, without a rename or a reload.
- [ ] **Constant size on screen.** A tag looks the same size at 3 m and at 39 m, and at any field of
      view; its text is sharp at both. It sits clear of what the avatar wears on its head, and follows
      the head through emotes with no lag.
- [ ] **Exactly one tag per player — no duplicates / orphans / pile-up.** None accumulate over a long
      session, after alt-tabbing away and back, or across a comms/livekit blip's leave→enter churn. A
      player who leaves takes their tag with them at once. `/nametag_state` counts the tags as
      `shown` and `hidden`: together, one per player with a profile.
- [ ] Tags show from 2 m to 40 m from the camera, **fading out** from 20 m, and not behind a wall.
      Your **own** tag shows above your head in third person and is hidden in first person.
- [ ] **Crosshair** shows when the camera is locked (mouse hidden) and hides when the cursor is free.
- [ ] **Hover / proximity prompts** ("Press E…") show on interactables and sit on the right entity.
- [ ] **`/goto` and `/goto-local`**: `/goto x,y` teleports inside the realm the player is in, Genesis City or a
      world, within that realm's bounds; `/goto world` and `world/x,y` travel; `genesis` and `main` are Genesis City, at Genesis Plaza when no parcel is named;
      `random` and `crowd` are Genesis City places from any realm; `/goto-local x,y` never changes realm. Each answers once in chat when the
      engine does (`🟢 You teleported to x,y.`, `🟢 Welcome to the … world!`, `🟢 Welcome to Genesis
      City!`), a failure in chat alone with no travel notice beside it, a parcel out of bounds or a
      bad destination at once. In a scene preview `/goto x,y` moves inside the preview, and
      `random` and `crowd` answer the engine's refusal to change realm. A `/goto` typed while a
      travel is under way is read against the realm travelled to; a travel it replaces answers
      `🔴 Error: The operation was canceled!`. A place inside the realm travelled to
      (`/goto world/x,y` on the way to that world, `/goto-local x,y`) is that travel asked for
      again, so a travel that fails moves nobody. A `/goto crowd` still reading the busiest scene
      gives way to a `/goto` typed after it. A destination ending in `/name` lands on the scene's
      spawn point of that name (the debug panel's Teleport section shows the name looked for and
      whether the scene had it). Tests: `library/ui/apps/hud/test/hosted/chatGoto.test.tsx` (the
      session), `library/explorer-core/features/chat/chatCommands.test.ts` (the destinations),
      `src/test/bridgeTravel.test.ts` (the bridge).
- [ ] **Chat bubbles**: a message opens its author's tag into a bubble with a tail for 5 s (a new
      message restarts it); a long message wraps and is cut at 100 characters with an ellipsis; a message that @-mentions you has a pink-to-purple
      ring. A message said in a **community** chat by a player standing nearby shows the community's
      name on a smaller, dimmer row above their name, cut short with an ellipsis (and over your own
      head for your own); a **DM** shows a green `DM` tag after the name, and your own names who it is
      for when they are around. The Chat bubbles and reactions setting (All / Nearby only / None) picks which show.
      A translated message replaces the bubble's text. `<b>` and `<i>` tags are left out of the
      bubble, which shows the words between them unstyled; a tag that styles no text (`<b></b>`, a
      closing tag with nothing open) is shown as typed, in the bubble and in chat, and a link next
      to a tag is coloured as chat links it.
- [ ] **Reactions**: an emoji picked from the chat's Reactions button rises from just over your
      nametag: it pops in small, rises about a metre while it sways, swells once and is gone within
      a second, drawn over the nametag and over whatever stands in front of it. Picks made one after
      another each rise, the bar staying open. Another player's rise over their avatar, a burst of
      them one every 0.2 s. None shows over an avatar a scene hides, over a blocked player, with
      nametags off, or, for other players, farther than 25 m from the camera; with the Chat bubbles
      and reactions setting at None none shows over anyone. The debug panel's "Situational reactions"
      section counts them and its buttons play one without the HUD.
- [ ] **Voice badges**: a talking player's tag shows three green bars swinging; your own shows three
      dots while your microphone is open and you are quiet. While a player whose nearby voice you muted
      (profile menu, Hush) is talking, their tag shows the red hushed icon in place of the bars, and
      nothing when they are quiet; Hear brings the bars back. The same player talking in a private call
      or community stream with you, which the mute leaves audible, shows the green bars.
- [ ] **Lobby stage** (`crates/system_ui/src/lobby_stage`, `bridge-scene/src/domains/avatarPreview.tsx`,
      lobby region): after sign-in the page's stand-in (the chosen look's backdrop + snapshot)
      crossfades to the engine's 3D stage: backdrop, floor dissolving into it, podium, props, mist,
      dust and vignette around the live avatar, which waves, plays an idle emote every 8–20 s and
      turns when dragged. Settings → Lobby, in the full page or the lobby's compact Settings, switches
      the look at once (Halloween 2026, Game World).
      Customize opens the Backpack modal: the lobby around it keeps the stage's last frame, the
      preview stays the Backpack's purple until the engine reports `avatarPreviewReady`, then shows
      the avatar on its podium, with nothing of the lobby stage or the world in between and nothing
      of the preview outside the modal; closing hands the live stage back. Jump In loads the world
      (the hold is released) and a failed trip returns to the lobby.
- [ ] **Minimap Camera style** (`bridge-scene/src/domains/minimap.tsx`, map region): picking Camera in
      Minimap settings, expanding the minimap in that style and entering a World each show the map
      square black until the engine reports `minimapReady`, then the top-down view, with nothing of
      the player's own view in the square in between. Switching back to Satellite or Parcel atlas
      shows the tiles at once.
- [ ] **Lobby page** (`features/lobby`): the widgets sit on the four corner grids as the stored layout
      says; edit mode (the pencil) frames them, drags them with the others making room, lists the
      ones off the grids in Customize Lobby, and Save keeps the layout across a reload while Cancel,
      Back and Escape drop the changes. From the startup lobby, Map, Places, Events and Go to pick
      where the world starts. VIEW ALL opens Places, Events, Communities and Gallery compact in the
      lobby's section modal: Communities four cards a row, Gallery five columns of 292x219, and a
      photo opened there covers the whole screen, its Jump In closing the photo and the modal. The
      profile menu's Settings opens there too: the full page's tabs and fields under its own header,
      on the tab last picked in either.
      A place card opens its detail: Set as Home presses at once and the landing card follows; a
      Genesis City place offers start navigation, which turns to exit navigation, and the route
      shows on the map once in the world; a World offers no navigation.
- [ ] **Sign-in stage** (`avatarPreview.tsx`, signin region): a new account's look and name screens
      show its avatar on the golden podium at the right, over the sign-in background (vignette and
      drifting logo pattern) drawn by the engine across the whole screen. The page's background fades
      out only once the engine reports `signinStageReady`, and comes back at once on Back. Picking a
      look or body type changes the avatar in place. Check a 16:9 window and a much wider or taller one.
      Mock has no engine, so the page's background stays.
- [ ] **Full-screen view entrance** (`features/menu/MainMenuShell`): opening Map, Backpack or any other
      menu page from the world darkens the world to black, then reveals the page, in 0.2 s; the
      Backpack's engine-drawn avatar appears only after the black frame. Switching between menu pages
      plays no black frame, nor does a first open of Create from another page when its code loads
      within 0.2 s; a menu closed and reopened within 0.2 s appears with no entrance. With reduced motion
      on, the page fades in without it.

---

## 5. Pre-merge review checklist

Run through this on every diff (it encodes `AGENTS.md`):

- [ ] **Design system** — no new hardcoded brand/status hex, `rgba()`, radius, or type size in a
      component (`grep` your diff for `#` and `rgba(`); each must be a `tokens.css` var or justified.
      No bespoke `<button>`/control where a `@hud/design` primitive exists.
- [ ] **No dead code** — no unused exports/CSS classes/imports; no commented-out blocks; comments are
      sparse and explain *why*, not *what*.
- [ ] **Structure** — one concern per file; reuse a primitive or *create* one rather than inlining a
      recurring pattern; bridge logic in one `domains/*` file.
- [ ] **Perf** — catalyst fetches cached (per address); per-frame engine systems are change-gated /
      throttled, not doing work every frame for free.
- [ ] **Tests** — tier 1 still green; **a new/changed domain has a tier-1.5 baseline**; world-space
      changes ran the §4 checklist.
- [ ] **Both projects typecheck** (`pnpm run typecheck` + `cd bridge-scene && pnpm run build`), and the
      bridge scene lints clean (`pnpm run lint` there).
- [ ] **No secrets / `.env` or working reports** staged; review the staged paths under the
      repository-root `AGENTS.md` rules. Use an imperative commit subject without AI attribution.

- [ ] **Input routing** — follows the §7 contracts (no hand-rolled Escape/hotkey handlers).
- [ ] **No native popups** — no `<select>`, no `<input type="date|time|color">`, no
      `alert`/`confirm`/`prompt`. On native the HUD is rendered offscreen and CEF paints popup
      widgets as a separate surface the engine never composites (`update_hud_texture` takes whatever
      paint arrives), so opening one replaces the whole HUD with the widget's bitmap. Use `Select`,
      `DateField` and `showConfirm`/`showDialog` from `@hud/design`.

---

## 6. Adding a domain

1. Build it (token-driven, reusing `@hud/design` primitives).
2. Add the mock data in `library/explorer-core/engine/mockBridge.ts` so `?mock=1` renders it richly.
3. Add a tier-1 test for its API contract + clicks beside the shared code: `library/explorer-core/test/`
   for core code, `library/ui/apps/hud/test/hosted/` for UI and session code (`src/test/` only for this
   host's own files).
4. Add a screenshot to `e2e/visual.spec.ts` and run `pnpm run test:visual:update`.
5. If it's world-space (engine-drawn), add a line to the §4 checklist instead of a screenshot.
6. List its expectations here.

---

## 7. Input routing contracts

The ENGINE resolves input to actions; the HUD reacts to the action stream and declares its
focus back (`uiFocus`). Keep it that way — the AZERTY bug and the CEF double-fire both came
from HUD-side key matching. The rules:

- **Never add a DOM hotkey or Escape handler.** Panel/menu keys arrive as `systemAction`
  messages (dispatcher in `useEngineSession`); add a `case` there. Cancel/close behaviour
  for leaf UI (a lightbox, an open dropdown) = register on the **cancel-layer stack**
  (`library/explorer-core/lib/cancelLayers.ts`) while open — the dispatcher peels popup → layer → panels, one
  per press, keyboard and gamepad alike.
- **The only DOM key handling allowed is inside a focused text field** (chat, map search),
  where the engine deliberately resolves nothing (`UiText`). There: match with
  `isCancelKey`, and `stopPropagation()` anything you consumed so the blur rule doesn't
  also fire on the same press.
- **A new panel/popup/overlay must be visible to `uiFocus`** — panels via the session's
  `anyPanelOpen`, dialogs via the popup stack (`openPopup`), full-screen input freezes via
  `inputLock`. If the engine can't see that your surface is active, scenes keep receiving
  input behind it.
- **The scene editor stands the HUD down through `session.editor.setMode`** (Create or `?editor`,
  `library/ui/apps/hud/features/editorHost`): in `edit` the dispatcher acts on `Cancel` only and App unmounts the
  sidebar, minimap, chat and pointer; `play` adds the reticle and the emote wheel. Not through
  `inputLock`: that declares `ui` focus, which would take the viewport's input from the editor's
  own scene. A new hotkey or persistent surface must respect the mode (`hudActsOn`).
- **Scrollable HUD content** just needs real `overflow: auto` — hover detection, the
  engine-side reservation and gamepad scrolling all key off that.
- Key **labels** come from `bindingLabels` (`keyHintFor`/`labelForInput`), never hardcoded
  letters — they follow the user's rebinds and keyboard layout.

## Portables

A portable experience runs beside the scene the player stands in: a smart wearable's scene
while it is worn, or a portable another scene spawned. The engine decides and lists them; the HUD
shows them and stops or restarts them (`bridge-scene/src/domains/portables.ts`). Expectations:

- **List**: the engine's `portables` message (`getPortablesStream`), sent on subscribe and on every
  change, never the HUD's own scene or a startup portable. No polling and no prompt of its own: a
  portable's permissions are asked on use through the scene permission dialog.
- **Rail**: the `Portable experiences` button shows while the list is non-empty; its tooltip reads
  `N running`, plus `, M paused in this scene` while the scene the player stands in disables
  portable experiences. Its popover lists one row per portable: name, kind (`Smart wearable` /
  `Portable experience`) and state (`Loading`, `Running`, `Failed: <reason>`, `Paused in this
  scene`, `Stopped`). `Stop <name>` on every row with a scene hash runs the console's
  `/stop_portable <hash>`; a stopped smart wearable (still worn) shows `Restart <name>` instead,
  `/smart_wearable restart <urn>`. The HUD's own command replies stay out of chat; a failed one is
  toasted.
- **Toasts**: entering running, paused, stopped or failed is toasted, from the list the player
  arrived with (no burst at login).
- Mock: `?mock=1&mockportables=1`. Tests: `library/ui/apps/hud/test/portables.test.tsx`.

## Reactions

A situational reaction is an emoji the player picks from the chat's Reactions button. The engine
shows it over their avatar and tells the players around them (`crates/avatar/src/reaction`); the
HUD offers the pick and shows each reaction the engine plays on screen
(`bridge-scene/src/domains/reactions.ts`). Expectations:

- **Button**: the heart in the chat's strip, named `Reactions`, pressed while its bar is open. It
  shows while the chat is open and not showing the nearby list, in every conversation, a blocked
  DM included. It types nothing into the message.
- **Bar**: the bar a message's React button opens (the fixed emoji, up to three recent ones, `+`
  for the full picker), named `Reactions`, over the button. A pick sends `sendReaction` and the
  bar stays for the next. The bar's picks leave the recent emoji alone; the full picker's become
  recent, and the picker stays open too. A press outside, the button again, another conversation,
  the nearby list, or the chat closing or giving way to a panel closes it; Cancel closes the
  picker, then the bar. It and a message's bar are never both open.
- **Chat Reactions switch**: in the bar, bound to the engine's `Chat Reactions` setting (off until
  the player turns it on; the Settings page lists it too). Absent on an engine with no such
  setting.
- **On screen**: while the setting is on the engine passes each reaction it plays (`reaction`),
  the player's own included, and each rises from beside the button for about two seconds
  (`features/chat/ReactionLane.tsx`, its numbers in `reactionMotion.ts`). It starts at the same
  place while the chat is collapsed and the button is not drawn, at every HUD scale. At most 30
  fly at once; more are not shown. The lane is part of the chat: while the chat gives way to a
  panel (Friends) or the HUD is hidden (a full-screen page, the camera, the scene editor, Hide
  UI) none is shown, the ones in flight go with it and none is shown later. An emoji the wire
  has no index for (a flag) plays nowhere, so the HUD shows none of its own accord. With reduced
  motion a reaction fades in place.
- Mock: `?mock=1&mockreactions=1` (the setting on and the nearby players reacting). Tests:
  `library/ui/apps/hud/test/hosted/chatReactions.test.tsx`, `chatReactions.clicks.test.tsx` and
  `chatReactionLane.test.tsx`, `library/ui/apps/hud/features/chat/reactionMotion.test.ts`,
  `src/test/bridgeReactions.test.ts`.
- No visual baseline yet: the bar (its row of emoji and the `Chat Reactions` row under it) needs
  a capture of the open chat with the bar up. The lane has none to take: it is motion only, the
  harness stops animations at capture, and a flight that is over draws nothing. Where a reaction
  starts and its scale are tier-1 (`chatReactionLane.test.tsx`); its look is checked by eye in
  the mock above.

## Analytics

The page reports the screens and clicks mobile does, with mobile's names and property shapes
(`lib/analytics.ts`). Nothing is sent from the page: each event is stamped when it happens, waits
in prefs (`PREF.analyticsQueue`, newest 200) and goes to the engine's `/track` console command once
the engine is up (`EngineDriver.launch` attaches the console); a send the console rejected is kept
for the next one. The engine batches the Segment posts, with its own events, and adds the common fields
as it does for those (`crates/analytics/README.md`), including `campaign_anon_user_id`, the
landing page's Segment anonymous id that the shell reads from the `ajs_anonymous_id` cookie and
hands the app as `anon_user_id` (`shell/main.ts`). The session id goes to the engine at launch (`EngineDriver.launch`), and the
return from the sign-in redirect continues it (the shell keeps it in sessionStorage and hands it
back as `analyticsSession`). Expectations:

- **Boot** (`bootAnalytics`, `main.tsx`): `App Opened` then `Screen Viewed` START, except on the
  return from the sign-in redirect. Between them, when the link carries a campaign (`utm_*`,
  `gad_*`, ad click ids; GA's cross-domain `_gl` alone does not count), `Install Attribution` in
  mobile's install-referrer shape, told apart from mobile's by `platform` `web` and
  `attribution_source` `url` (one per page load with a campaign, not per install): `referrer` the
  raw campaign params, the `utm_*`, `gclid`, `gbraid` and `wbraid` columns, and `page_session_id`,
  this load's session (mobile's column). Every page event carries `session_id`, the session of the
  load that created it: the queue outlives a load that bounces before the engine is up, and the
  engine keeps the page's value over its own, and drops an event held over 36 h by its own time
  (`crates/analytics/README.md`).
- **Sign-in link** (`authLoginUrl`): the entry's `utm_*` params ride along, so the auth site's
  own Segment calls carry the campaign and tie it to the address it signs in. Once read, the
  shell and the app take the attribution params (`utm_*`, `gad_*`, ad click ids such as `gclid`,
  the landing page's anonymous id) off the address bar (`dropAttributionParams`), so a copied link
  or a reload does not attribute again; the Google tag gets them from what the shell kept at load.
- **Sign-in** (`useEngineSession`): with no session the first screen (`features/startup`) is
  ACCOUNT_HOME, its PLAY AS GUEST a PLAY_GUEST click and LOGIN
  OR SIGNUP a SIGN_IN click; the guest account is created only on that click. After any sign-in,
  Verifying shows until the account's welcome answer (an existing account's terms are accepted for
  it); a new account (AVATAR_NAMING) picks a starting look and a name, deploys on JUMP IN and goes
  straight in, AUTH_DEPLOY_SUCCESS. LOGIN OR SIGNUP opens the methods screen: Google,
  MetaMask and More options leave for the sign-in page with `?loginMethod=` (SIGN_IN, AUTH_BROWSER_OPEN
  {method}); an email code is sent and checked in the HUD (the web shell or the bridge scene makes
  the thirdweb calls) and its account signs in as fully_registered. COMEBACK (after a failed stored
  sign-in), ACCOUNT_GUEST_CREATE, AUTH_BROWSER_OPEN, AUTH_SUCCESS {login_type}, AUTH_ERROR
  {error}. `Guest Wallet Creation` wraps the
  thirdweb guest step (`features/auth/guestWalletEvent.ts`). A stored sign-in with a day or less
  left is forgotten at startup (`getStoredLogin`), so the player gets the first screen and the auth
  site asks for a new sign-in; a run already going keeps its sign-in until it expires.
- **Welcome / lobby**: WELCOME or AVATAR_NAMING, AUTH_DEPLOY_SUCCESS; click LETS_GO (JUMP IN on the
  name screen); LOBBY, PLACES_PICKER; clicks JUMP_IN {kind, place_id|event_id}, CUSTOMIZE,
  NOTIFICATIONS, CLOSE (`LobbyHome.tsx`). The lobby's own events, named as the Unity explorer
  reports them so both clients' lobby data reads alike (`features/lobby/lobbyAnalytics.ts`):
  `lobby_opened` and `lobby_closed` {source,
  is_startup, duration_sec}; `place_card_clicked` and `place_jumped_in` {place_id, place_name,
  place_coords, highlighted, from_section}; `event_card_clicked` and `event_jumped_in` {event_id,
  event_name, event_coords, highlighted, from_section}; `jump_to_friend_clicked` {receiver_id,
  friend_position}; `places_section_opened`, `events_section_opened`.
- **Loading**: the engine's, not the page's. LOADING_START {position, realm, when}, LOADING_END
  {position, status} and LOADING_TIMEOUT after 90 s follow its own loading screen
  (`crates/analytics/src/loading.rs`), the one `sceneLoading` mirrors here.
- **Social**: Friend Request, Friend Accept, Unfriend, Block User on the action.
- **Nearby voice**: `nearby_voice_user_mute` {wallet_id, is_muted} on the profile menu's Hush or
  Hear; not for the mutes read from the saved list, nor for a Hush or Hear the list already has.
- **In world**: the engine's First Move In World (an `analytics` message over the bridge, which
  relays the engine's own Segment events by name; a metre walked since logging in), once per page load, with the terms accepted (the welcome
  flow's answer, or the engine's persisted config), posts `bevy-shell:in-world` {login_type,
  session_id} to the shell, the one message that leaves the page for anyone but the engine. On decentraland.org
  only, never for a `preview=true` page, once per tab session, the shell then loads the Google
  tag for the Explorer GA4 stream and sends `bevy_first_move_in_world`, the Google Ads conversion,
  with the user property `login_type` `guest` or `web3`, the event param `dcl_session_id` (the
  Segment session), and as its page the url the tab arrived
  with, keeping only its `utm_*` and ad click ids (`shell/googleTag.ts`). Nothing of Google's loads before that moment.
  Once gtag.js is up the shell posts GA4's ids back (`bevy-shell:google-tag` {client_id, session_id}),
  and the app sends them as `Google Tag Linked` {ga_client_id, ga_session_id (null when GA4 gives
  none)}: the GA4 export's `user_pseudo_id` / `ga_session_id` on the Segment session, taken at the
  conversion (`shell/googleTag.ts` `onInWorld`, `listenForGoogleTag` over `lib/shell.ts`
  `onShellMessage`).
- Tests: `library/explorer-core/test/analytics.test.ts` (stamping, the handoff, the rejected send, `Google Tag Linked`),
  `library/explorer-core/test/analytics.boot.test.ts` (the boot events, `Install Attribution`),
  `library/ui/apps/hud/test/hosted/analytics.session.test.tsx` (the sequences above), `engineDriver.test.ts` (the session
  id at launch), `entryParams.test.ts` (campaign params).

## Photo mode

The camera reel's in-world camera (`features/photoMode`, bridge `domains/photoMode.ts`). The engine
owns the camera, the capture and the upload and reports them on the `photoMode` stream; the HUD
owns the mode as a surface. Expectations:

- **Opening and closing** go through the session: the `PhotoMode` key (a `systemAction`, so the
  dispatcher's popup, editor and lobby gates apply), the rail's Camera button and the overlay's
  Close and Gallery send `setPhotoMode` with its source (`shortcut` or `button`, the engine's
  analytics). The gallery key and button close photo mode on the way to the gallery; Escape is the
  overlay's cancel layer. Opening closes every page and panel.
- **Focus**: photo mode is part of `uiFocus`, so scenes get no input while it is on and the engine
  reads only the camera's own keys above that. A popup opening on top of it closes it; a scene
  permission prompt waits until it ends (`App.tsx`).
- **The overlay** replaces the chrome (no rail, minimap or chat): the frame at 0.87 of the tighter
  screen dimension in 16:9, the thirds grid, Gallery / Shoot / camera controls under it, the camera
  controls panel whose keys follow the bindings (scene movement actions included), and the
  storage-full notice in Shoot's place. Shoot is disabled while a photo is in flight, the storage
  is unknown or the capture effect plays; a capture plays the flash once per event seq and the
  photo flies into the Gallery button.
- Tests: `library/ui/apps/hud/test/hosted/photoMode.test.tsx` (the session contract above), `library/ui/apps/hud/test/hosted/photoModeOverlay.test.tsx`
  (the overlay's buttons, bindings, notice and effect); the e2e camera scenarios
  (`apps/bevy-explorer/e2e/specs/camera.test.ts`) drive the real engine.

## Scene item purchase

A scene's request to open the purchase confirmation for one item (`openExplorerUi` with
`EU_ITEM_PURCHASE`; bridge `domains/itemPurchase.ts`, `features/itemPurchase`). The engine applies
the gates and parses the item; the HUD draws nothing of its own. Expectations:

- **Answered on receipt.** The engine holds the scene's call, and every other scene's request,
  until the HUD says `accepted` or `unavailable`, with no deadline. The session answers in its
  message handler (`useEngineSession.ts`), never after a read, a render or an effect.
- **A page that connects settles the request in flight** (`domains/itemPurchase.ts`): the bridge
  sends a request no page answered again on `hello`, and reports one a previous page accepted
  `dismissed`. The session answers a repeated request id once.
- **Availability is the host's.** A host that can buy passes `purchaseSceneItem` to `HudApp`
  (`/play` in the web shell, with the in-world Shop on), which hands it to the session; without
  it every request is `unavailable`. This host passes none.
- **One outcome.** An accepted request is reported once more: `purchased`, `dismissed` or
  `failed`. A request with no item is `accepted` then `failed` with nothing opened. A purchase
  that rejects, or throws before returning a promise, is `failed`.
- **The confirmation is the shell's** purchase dialog, over the HUD frame; there is no HUD page,
  popup or screenshot for this domain.
- Tests: `library/ui/apps/hud/test/hosted/itemPurchase.test.tsx` (the session contract and the
  answers above); `src/test/bridgeItemPurchase.test.ts` (the relay and what `hello` settles); `apps/ui3/src/play/test/scenePurchase.test.ts` (what the shell is asked and how
  its operation's states map to the outcome); the engine's gates and request lifecycle in
  `crates/restricted_actions/src/item_purchase.rs`.

## Preview texture optimization

A scene preview served by `dcl-scene-cli start` reports its background texture import on the
preview socket; the engine relays it (`textureOptimization`, from `getTextureOptimizationStream`,
`bridge-scene/src/domains/textureOptimization.ts`) and the shared HUD's
`library/ui/apps/hud/features/preview/textureOptimizationToast.ts` shows it as one toast per scene, under its own key. Expectations:

- With the project's setting on: `running` updates one busy progress toast in place, with no
  Dismiss (the next update would bring it back), until 60 s pass with no update (the CLI stopped,
  the socket dropped or the player left the realm), when it goes and the scene's progress is
  forgotten; `ready` shows
  the size saved and **Reload** (accessible name "Reload scene to see optimized textures"), which
  posts `reloadScene` with the status's scene id; `failed` says how many textures stay PNG, or that
  the import failed, with Dismiss and no action. With it off, nothing shows.
- A `ready` marked `replay` (the preview server's last status, sent again as the socket connects)
  offers **Reload** only when the HUD showed that scene's progress, since the run ended while the
  socket was down. Otherwise it leaves the toast as it is: the scene was loaded from those files.
- `?mock=1&textures=1` plays a run and the Reload offer.
- Tests: `library/ui/apps/hud/test/textureOptimization.test.tsx` (the Toast primitive's keyed, persistent and action
  toasts; each state; the replay rule; the stale progress timeout; one toast per scene; Reload
  through the session),
  `src/test/bridgeTextureOptimization.test.ts` (the relay).

## Gallery (camera reel)

The Gallery page and the passport's Photos tab (`library/ui/apps/hud/features/gallery`, bridge `domains/gallery.ts`).
Expectations:

- **Paging**: a reel is read 100 photos a page, the next page when fewer than 12 loaded photos remain
  past the last one on screen, and paging stops once the reel's total is loaded or the service sends
  a short page (counted before url-less images are dropped) (`useReelList`). Every Gallery open reads
  again from the top. Your own reel goes through the bridge, signed (private photos included), its
  replies matched by `requestId` (`bridgeCall`). The passport reads the PUBLIC reel from the HUD
  itself, unsigned (`publicReels.ts`), since every bridge request is signed; `?mock=1` swaps in the
  mock bridge's reels.
- **Failures**: a failed first page shows an error with RETRY, never the empty reel; a failed later
  page ends the grid with a RETRY row, and paging waits for it instead of asking on every scroll.
- **The grid** (`ReelGrid`) mounts only the rows near the viewport; it reports what is on screen only
  once it has measured it. Gallery: 6 x 306x230 with month headers; passport: 4 columns at 272:201,
  no headers. A thumbnail is disabled until its image has loaded.
- **The options menu** (`design/AnchoredMenu` around `ContextMenu`): Set as Public, Share on X, Copy
  Link, Download, Delete, in that order (`reelActions.ts`, `useOwnReelActions.ts`). The passport offers
  it only on your own passport, and making a photo private there drops it at once (back in place if
  the write fails). It is portaled with `--ui-scale`, opens below-right of the ⋮ and flips or clamps
  at the screen edges: check it visually near the edges. On native, Download needs the engine's CEF
  download handler (`crates/cef_offscreen`): it saves to `Downloads` under the home folder and
  reports how each save ended, and the toast waits for that report. That toast shows the folder,
  which opens in the file manager on a click (once per toast); web has no such link. Share on X on native asks the
  bridge scene to open the post in the user's browser (`openExternalUrl`, `https:` only); on web
  it opens a tab.
- Tests: in `library/ui/apps/hud/test/`, `hosted/gallery.test.tsx` (the session calls),
  `hosted/gallery.page.test.tsx` (paging, failures, the menu and its actions),
  `profile.passport.test.tsx` (passport photos) and `reelGrid.test.tsx` (rows, virtualization, menu
  placement); here, `src/test/bridgeGallery.test.ts` (the bridge's requests and replies).

## Map

The full-screen map (`library/ui/apps/hud/features/map`, core in `library/explorer-core/features/map`).
Expectations:

- **Signed reads and writes go through the bridge:** places by parcel, the destinations search, likes,
  favourites and a place's photos in `domains/places.ts`; events at parcels and reminders through the
  events domain's own `getEvents` and `setEventAttending`. Each request kind has its own answer kind
  that echoes `request` (a reminder's names its event), and every request is answered, a failed one
  with `failed`. The points-of-interest list and the connected players are plain fetches.
- **Ids:** the map and the Events page both send `getEvents`; their ids come from one sequence
  (`engine/requestIds.ts`), so each takes only the answers to its own requests.
- **Nothing draws per frame:** the camera writes custom properties on the viewport; the sea is one
  image (`assets/map/sea.webp`) placed in the world layer like a satellite tile.
- **Home:** the map and the place detail read the session's home and set it with `setHomeScene`
  (`placeHomeTarget` in `features/map/home.ts` picks what each saves); Set as Home on the place
  that is home clears it with `clearHomeScene`, as the minimap's switch does. The home is the
  signed-in account's: `homeScene` says `pinned: false` for an account with none, which then has
  no pin and starts on 0,0 of the default realm. Until the engine answers, the home is the web
  build's boot copy, its default realm read as Genesis City, so the startup lobby agrees with
  the world; the lobby asks again once it shows, as the account may not be the boot copy's.
- **Arrival** ends a route only in Genesis City: a World's parcels share the numbers, not the place.
- **Quest pins:** the engine streams the map pins of portable experiences (`GetMapPinsStream`, the
  whole list on each change, the last list again on `hello`), and the session drops them when it ends.
  The Quest Mini-Games layer, on by default, draws them on the map and on the Genesis City minimap; a
  pin outside the city is dropped. Every pin draws the default quest pin: the scene's own icon is
  not read. Clicking one selects its parcel, it never teleports. `?mock=1&mockpins=1` shows two pins.
- Tests: `library/explorer-core/test/{navmap,mapCore}.test.ts`, `library/ui/apps/hud/test/hosted/map.page.test.tsx`,
  `hosted/home.session.test.tsx`, `hosted/placeDetail.test.tsx`, `mapEventInfo.test.tsx`, `mapPins.test.tsx`; here,
  `src/test/bridgePlaces.test.ts` and `bridgeEvents.test.ts`. Visual baseline: `panel-map`.

## Donate

Tipping a place's creator in MANA, where the host can sign a tip (`@hud-host`'s `donations`: the Sites `/play`
web shell; null here, so this HUD shows no Donate). The shared HUD names a place and nothing else
(`features/donate/useDonate.ts`); the shell resolves the creator, draws the panel and signs (`apps/ui3/src/play/shell/donate`,
[ADR](../../../../docs/adr/2026-11-12-play-shell-signs-tips.md)). Expectations:

- **Where it shows:** the minimap's header for the scene the player stands in (the session's
  `minimap.donatePlace`: a Genesis City parcel or a World, none in a preview or a local realm), and the
  map's place panel. Only when the host says the session may tip and the place has a creator; hidden while
  the first answer is pending. Whether the session may tip is asked once per sign-in. The minimap's place
  changes with the scene, not with each parcel crossed (none on a parcel with no scene), and its last answer
  stays while the next scene is asked about; on the map a newly picked place hides it until answered.
- **After a tip** the host's `onSent` plays the `money` base emote; a `tip_received` notification opens the
  tipper's passport (`notifications/summary.ts` `tipTarget`).
- **SEND TIP waits:** the form opens with the first suggestion picked, so it is ready to send at once; SEND TIP
  answers only `ARM_MS` (500 ms) after the form appears (`design/ArmedButton`), so a second click of the press that
  opened it sends nothing.
- **Telling the creator:** once the tip is mined the shell forwards its hash to the catalyst
  (`features/donate/tip.ts` `forwardUntil`). A 429, a 5xx or no answer is asked again with backoff, never sooner than
  the server's Retry-After, until the delivery window ends; a 503 for a lagging index carries a Retry-After and is
  asked again too. A 503 coded `tip-index-unavailable` (the node has no MANA transfer index, its cursor lacks the
  whole history, or nothing has advanced it for hours) and any other 4xx end the forward at once;
  the shell only logs that outcome, and the tip itself stays sent.
- Tests: `library/ui/apps/hud/test/donate.test.tsx` (gating, open, emote, notification),
  `library/ui/apps/hud/test/hosted/donateSend.test.tsx` (SEND TIP's wait),
  `library/explorer-core/test/donatePlace.test.ts`, `library/explorer-core/test/donateTip.test.ts` (the forward's
  retries and stops); the shell's side in `apps/ui3/src/play/test/donateShell.test.ts`
  and `donateGuard.test.ts`. No visual baseline: the panel is drawn by the `/play` shell, not by `?mock=1`.

## Gifting

Sending an owned wearable or emote to another player, where the host can sign the transfer (`@hud-host`'s
`gifting`: the Sites `/play` web shell; null here outside `?mock=1`, whose fake host sends nothing). The shared HUD
draws the picker and names a token and a recipient (`features/gifting`); the shell checks both, asks the player to
confirm them in its own overlay, draws the transfer and signs (`apps/ui3/src/play/shell/gift`, [ADR](../../../../docs/adr/2026-11-16-play-shell-signs-gifts.md)).
Expectations:

- **Where it shows:** the profile card of chat names, nametags and the friend row (after View Profile), the
  community card's member menu (after Call; greyed for a blocked member, either way), and another player's
  passport options (before Block). Not in the voice-stream panel's menu, not on your own card, not for a guest sender. A guest recipient is offered it, as in unity-explorer.
  Whether the session may gift is asked as each of those surfaces opens, as Donate's host is asked (`useGiftingAvailable`); nothing is cached.
- **The picker** reads 16 owned on-chain items a page through the bridge's `giftingQuery`, newest first, with the
  search on the server. A pending gift doesn't count as a copy; an equipped single copy is covered and can't be
  picked; SEND GIFT sends the newest-transferred copy neither worn nor pending, and refuses an item that isn't a
  Polygon collection item with a toast.
- **The shell's SEND GIFT waits:** on its confirm step (`GiftTransferPanel`) it answers only `ARM_MS` (500 ms) after
  it appears (`design/ArmedButton`), so a second click of the picker's SEND GIFT sends nothing
  (`test/hosted/giftTransferPanel.test.tsx`).
- **A `transfer_received` notification** opens the gift-received popup (`features/gifting/GiftReceivedPopup.tsx`),
  on every host: the item comes from the notification's token URI.
- Tests: `library/ui/apps/hud/test/hosted/gift*.test.tsx`, `library/explorer-core/test/gift*.test.ts`,
  `src/test/bridgeGifting.test.ts`; the shell's side in `apps/ui3/src/play/test/giftShell.test.ts`, `giftGuard.test.ts`
  and `giftProtocol.test.ts`. The picker shows in `?mock=1`; the transfer panels are drawn by the `/play` shell.
