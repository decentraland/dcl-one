> Detect whether your scene is running on mobile, desktop, or web.

# Detect the Platform from Code

Use the `isMobile()` function to adapt your UI, controls, and gameplay specifically for mobile. This is the recommended way to deliver a great experience across all clients without forking your scene logic entirely.

## Available functions

```ts
import { getPlatform, isMobile, isDesktop, isWeb } from '@dcl/sdk/platform'
```

* **`getPlatform()`** — returns the current platform as `'mobile' | 'desktop' | 'web' | null`. Returns `null` until the explorer has reported its platform back to the scene (this happens shortly after the scene starts).
* **`isMobile()`** — returns `true` if the player is on the mobile client.
* **`isDesktop()`** — returns `true` if the player is on the desktop client.
* **`isWeb()`** — returns `true` if the player is on the web client.


> [!WARNING]
> **📔 Note**: These functions read a value that is populated asynchronously when the scene starts, and nothing guarantees it's already available by the time `main()` runs. If you call them too early, they may still return `null` / `false`. To be safe, defer any platform-dependent setup until `getPlatform()` returns something other than `null`, for example by checking inside a system:
> 
> ```ts
> import { engine } from '@dcl/sdk/ecs'
> import { getPlatform } from '@dcl/sdk/platform'
> 
> function platformCheckSystem() {
>   if (getPlatform() === null) return
>   engine.removeSystem(platformCheckSystem)
>   setupUI()
> }
> 
> engine.addSystem(platformCheckSystem)
> ```


## Branch your UI by platform

A common pattern is to set up a different UI for mobile and desktop players:

```ts
import { isMobile, isDesktop } from '@dcl/sdk/platform'

function setupUI() {
  if (isMobile()) {
    // Larger buttons, simpler layout for touch
    createMobileUI()
  } else if (isDesktop()) {
    // Denser layout tuned for keyboard and mouse
    createDesktopUI()
  }
}
```

You can also use the same pattern to:

* Show or hide on-screen instructions tailored to each input method.
* Replace small clickable elements with larger touch targets on mobile.
* Disable input bindings that are not easily available on mobile (see [Input on mobile](/docs/creator/build-for-mobile/develop/input-on-mobile/)).

## Checking the raw platform value

If you need to handle multiple platforms in a single switch, use `getPlatform()`:

```ts
import { getPlatform } from '@dcl/sdk/platform'

const platform = getPlatform()

switch (platform) {
  case 'mobile':
    // mobile-specific behavior
    break
  case 'desktop':
    // desktop-specific behavior
    break
  case 'web':
    // web-specific behavior
    break
  case null:
    // platform not yet known
    break
}
```

## Related

* [Mobile safe area](/docs/creator/build-for-mobile/develop/safe-area/)
* [UI best practices for mobile](/docs/creator/build-for-mobile/develop/ui-best-practices/)
* [On-screen UI](/docs/creator/sdk7/2d-ui/onscreen-ui/)
