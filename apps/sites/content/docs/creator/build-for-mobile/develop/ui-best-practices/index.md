> DOs and DON'Ts for designing scene UIs that work well on mobile.

# UI Best Practices for Mobile

There is no single proven recipe for Decentraland mobile UI yet — the platform is new and we are still iterating. The recommendations on this page are the current best practices, distilled from work on real scenes. Treat them as a starting point and test on a real device.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/5OmTTzpPdDc" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="https://www.youtube.com/watch?v=5OmTTzpPdDc">Watch on YouTube</a></p>



## DOs

* **Design mobile-specific UIs**, or vary your UI by screen size and platform. Use [`isMobile()`](/docs/creator/build-for-mobile/develop/detect-platform/) to branch.
* **Keep critical UI inside the [safe area](/docs/creator/build-for-mobile/develop/safe-area/).** The device's hardware margins (notch, status bar, home indicator) are cleared for you by default — just don't opt out with `screenInset: 'none'`. The client's own controls are a separate area, and `screenInset: 'interactable'` covers part of that job: it positions your UI in the rectangle the explorer designates for scene UI. On mobile that clears the left-hand controls; the action buttons on the bottom right are drawn over the area by design, so keep [those corners](/docs/creator/build-for-mobile/develop/safe-area/#where-the-client-controls-live) in mind. `'interactable'` requires mobile client `1.12.1` or newer — older clients report no margins at all and the UI covers the whole screen.
* **Minimize options.** Show only what the player needs right now and progressively disclose the rest.
* **Place actionable dialogs at the center of the screen** — anywhere a player needs to read and respond.
* **Place non-actionable messages at the top-center** — status, notifications, and ambient information.
* **Hide the touch controls behind a full-screen UI.** While a scoreboard, a shop, or a results panel is open, the player doesn't need to jump or press E. See [Full-screen UI over the action buttons](#full-screen-ui-over-the-action-buttons).

## DON'Ts

* **Don't size UI elements purely in pixels without a virtual screen in mind.** Pixel values are scaled against the `virtualWidth` / `virtualHeight` reference resolution described in [On-screen UI](/docs/creator/sdk7/2d-ui/onscreen-ui/#screen-virtual-scale) — `1600x720` by default on mobile, `1920x1080` on desktop and web. Know which reference resolution your pixel values are authored against, pass it explicitly if it isn't the default, and pair it with platform-aware sizing. Only disable the virtual screen (by passing a size of `0`) if you genuinely want raw canvas pixels.
* **Don't place elements outside the safe area.** They will clash with the client's controls.
* **Don't anchor UI to the top-left or the left edge.** That's where the client draws chat, the profile and the virtual joystick, all on top of your scene UI. Anchor right or center instead, or use `screenInset: 'interactable'`. See [Never anchor your UI to the top-left](/docs/creator/sdk7/design-experience/ux-ui-guide/#never-anchor-your-ui-to-the-top-left).
* **Don't put interactive elements in the bottom-right corner.** The action buttons are drawn over that region, and taps there go to the client's buttons, not to your scene.
* **Don't rely on small buttons.** Small targets are unreliable to tap on a touch screen.
* **Don't bind key actions to `IA_ACTION_3`–`IA_ACTION_6`** (the `1`/`2`/`3`/`4` keys on a keyboard). They are not easily reachable on mobile. See [Input on mobile](/docs/creator/build-for-mobile/develop/input-on-mobile/).

## Full-screen UI over the action buttons

`screenInset: 'interactable'` keeps your UI clear of the left-hand controls, but the action buttons in the bottom-right are drawn over that area by design. A panel that fills the area is still covered there, and taps in that region trigger the client's buttons instead of your UI.

For a **gameplay HUD** that must stay usable while the player moves, keep it out of the lower-right corner: anchor it top-right or top-center.

For a **UI that interrupts gameplay**, such as a scoreboard, a shop, or a results screen, hide the touch controls while it's open and restore them on close:

```ts
import { TouchScreenControls } from '@dcl/sdk/ecs'

function openScoreboard() {
  TouchScreenControls.hideAll()
  TouchScreenControls.hideJoystick()
  // ... show your UI
}

function closeScoreboard() {
  TouchScreenControls.showAll()
  TouchScreenControls.showJoystick()
  // ... hide your UI
}
```

`showAll()` only restores the gamepad buttons, so pair it with `showJoystick()`. Both helpers do nothing on desktop, so there's no need to branch on the platform. See [On-screen Controls](/docs/creator/sdk7/interactivity/touch-screen-controls/) for the full component.

If the avatar should also stop moving while the panel is open, add an [input modifier](/docs/creator/sdk7/interactivity/avatars/locomotion/#freeze-the-player) on top.

## Sizing

The old rule of thumb — *design on desktop, then scale UI sizes by 3× for mobile* — predates two SDK changes and should no longer be applied blindly. Most of that 3× is now applied for you:

* **`devicePixelRatio` takes no part in UI layout.** Pixel-sized UI now renders `devicePixelRatio` times larger than it used to — typically **2–3× on a phone**, and on retina laptops too. There is no opt-out.
* **The virtual screen defaults per platform**, `1600x720` on mobile against `1920x1080` on desktop and web, so the same pixel value already covers about 1.2× more of the screen on a phone.

Start from your desktop sizes unchanged, measure on a real device — see [Preview on mobile](/docs/creator/build-for-mobile/develop/preview-on-mobile/) — and scale up only where tap targets or text actually come up short. Branch with [`isMobile()`](/docs/creator/build-for-mobile/develop/detect-platform/) when the two genuinely need to differ.

## Related

* [Missing Features](/docs/creator/build-for-mobile/mobile-client/missing-features/)
* [Mobile safe area](/docs/creator/build-for-mobile/develop/safe-area/)
* [Detect the platform from code](/docs/creator/build-for-mobile/develop/detect-platform/)
* [On-screen UI](/docs/creator/sdk7/2d-ui/onscreen-ui/)
* [UX & UI Guide](/docs/creator/sdk7/design-experience/ux-ui-guide/)
