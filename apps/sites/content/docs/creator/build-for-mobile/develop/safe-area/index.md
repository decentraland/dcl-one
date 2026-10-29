> Where scene UI can safely live on mobile — clear of the device's hardware margins and of the client's own controls.

# Mobile Safe Area

On a phone, two unrelated things eat into the screen space your UI can safely use:

* **The device's hardware margins** — the notch or camera cutout, the status bar, the home indicator, and rounded corners. UI drawn underneath them is partly hidden or hard to tap.
* **The Decentraland client's own controls** — joystick, chat, profile, camera controls, action buttons. Scene UI placed under them clashes visually and competes for the same taps.

You don't have to measure either one. The client reports both areas at runtime and the SDK positions your UI inside the one you pick, with the [`screenInset`](/docs/creator/sdk7/2d-ui/onscreen-ui/#screen-inset-area) renderer option:

| What you want | What to pass |
|---|---|
| Clear of the hardware margins | nothing — `screenInset: 'device'` is the default |
| Clear of the hardware margins **and** the client controls | `screenInset: 'interactable'` |
| The whole screen, margins are yours to handle | `screenInset: 'none'` |

```ts
// Recommended for scene UI that must not clash with the mobile controls
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'interactable' })
```


> [!WARNING]
> **Client support**: `'interactable'` needs mobile client `1.12.1` or newer. On older clients the area is reported as zero and the UI falls back to covering the whole screen. That same release normalizes the `'device'` area between Android and iOS.
> 
> **It is not a no-op on desktop** — the desktop client reserves roughly the left 25% of the screen for its own UI, so `'interactable'` changes your desktop layout too. Branch with [`isMobile()`](/docs/creator/build-for-mobile/develop/detect-platform/) if you only want the inset on phones.


See [Screen inset area](/docs/creator/sdk7/2d-ui/onscreen-ui/#screen-inset-area) for the full reference: the three areas side by side on a real device, per-renderer behavior, and how they relate to the [virtual screen](/docs/creator/sdk7/2d-ui/onscreen-ui/#default-virtual-size).

## Where the client controls live

`'interactable'` gives you the area **each explorer designates for scene UI** — that is deliberately not a promise that every client control is outside it. On the mobile client it is the device safe area minus the left-hand column (chat, profile, joystick, emotes); the action buttons on the bottom right are drawn over the area **by design**.

<figure><img src="/docs/.gitbook/assets/screeninset-interactable.webp" alt="Scene UI inset to the area the mobile client reports as free of its own HUD"><figcaption><p><code>screenInset: 'interactable'</code> on mobile client <code>1.12.1</code>. The magenta rectangle is the scene UI, green outlines the interactable area. The left-hand column is excluded; the action buttons on the bottom right sit over the area.</p></figcaption></figure>

So even with `'interactable'`, treat these as crowded:

* **Bottom right** — the action buttons and the interaction button are drawn on top. Anything you place here is still reachable, but it competes for taps.
* **Top right** — profile and camera controls sit just outside the area, so UI hugging that corner reads as part of the client's HUD.

What the area covers is each explorer's call and can change between versions. Read it at runtime, never hardcode it, and check it on the platforms you target.

## Where to put scene UI

* **Center of the screen** — actionable dialogs, anything the player needs to read and respond to.
* **Top-center** — non-actionable messages, status, and notifications.
* **Center-bottom, above the interaction button** — context-sensitive hints.
* **Not the bottom-right corner** — even inside the interactable area, it belongs to the action buttons.

## Wrap part of your UI instead

If you opted out with `screenInset: 'none'` and want to protect only part of your UI, wrap it in the `ScreenInsetArea` or the `InteractableArea` component. Each reads its matching area and applies it to its children. Import them from `@dcl/sdk/react-ecs`:

```tsx
import ReactEcs, { ReactEcsRenderer, UiEntity, ScreenInsetArea } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export function setupUi() {
  ReactEcsRenderer.setUiRenderer(() => (
    <ScreenInsetArea>
      {/* A child sized 100% × 100% fills the safe inset area exactly */}
      <UiEntity
        uiTransform={{ width: '100%', height: '100%' }}
        uiBackground={{ color: Color4.create(0, 0, 0, 0.5) }}
      />
    </ScreenInsetArea>
  ), { screenInset: 'none' })
}
```

Both components position themselves absolutely using the values the client reports, so the `positionType` and `position` fields of their `uiTransform` are reserved — any value you set for them is ignored. All other `uiTransform` properties (`padding`, `flexDirection`, `alignItems`, …) and UI components (`uiBackground`, `onMouseDown`, …) work as usual. They react automatically when the area changes, for example on rotation or when system bars appear or hide, and they compensate for the [UI scale factor](/docs/creator/sdk7/2d-ui/onscreen-ui/#screen-virtual-scale), so the margins land correctly whatever your virtual screen size is.


> [!WARNING]
> **📔 Don't apply the inset twice:** the `screenInset: 'none'` in the snippet above matters. Wrapping your UI in a component while the renderer is also applying the matching inset pushes your UI inwards by double the margin. Pick one or the other.



> [!NOTE]
> **📱 Mobile only:** the device insets only have real values on the **mobile client**. On the **desktop client** they are `(0, 0, 0, 0)`, so `screenInset: 'device'` and the `ScreenInsetArea` component have no effect there and your UI renders exactly as it would without them. The interactable area, unlike the device insets, *is* non-zero on desktop.


## Why it matters

Scene UI that overlaps the client's controls will:

* Be partially hidden behind the joystick, interaction button, or camera controls.
* Compete for taps with those controls — players will accidentally trigger one or the other.
* Make your scene feel broken on mobile, which hurts featuring and retention.

Always verify on a real device using the [preview QR code](/docs/creator/build-for-mobile/develop/preview-on-mobile/).

## Related

* [Screen inset area](/docs/creator/sdk7/2d-ui/onscreen-ui/#screen-inset-area) — the full `screenInset` reference.
* [UI best practices for mobile](/docs/creator/build-for-mobile/develop/ui-best-practices/)
* [Detect the platform from code](/docs/creator/build-for-mobile/develop/detect-platform/) — use `isMobile()` to swap layouts.
* [On-screen UI](/docs/creator/sdk7/2d-ui/onscreen-ui/)
* [UX & UI Guide](/docs/creator/sdk7/design-experience/ux-ui-guide/)
