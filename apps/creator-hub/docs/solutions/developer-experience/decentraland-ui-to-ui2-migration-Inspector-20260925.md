# Removing decentraland-ui from the Inspector

The Inspector no longer depends on `decentraland-ui` v6 (the Semantic UI library) and imports nothing from `decentraland-ui2`. This document started as a plan to move the Inspector onto `decentraland-ui2`. That plan was dropped: the Creator Hub UI is moving to the plain-CSS components of `apps/ui3`, so the Inspector uses those components directly. The record below says what replaced each use and what to check when you touch the same area.

## What replaced what

| Before (`decentraland-ui`) | Now |
| --- | --- |
| `Loader`, `Dimmer` | The ui3 `Spinner` (`@ui/atoms/Spinner`), wrapped by `components/Loading`. |
| `Popup`, `PopupContent` | The shared ui3 `Tooltip` (`@ui/atoms/Tooltip`). |
| `WearablePreview` | `components/WearablePreview`, a local component. |
| `lib/styles.css`, `lib/dark-theme.css` | `theme/base.css` and seven custom properties in `theme/vars.css`. |
| `AlertColor` (type, from `decentraland-ui2`) | A local union in `hooks/useSnackbar.ts`. |

The `@ui` alias points at `apps/ui3/src`. `build.js` bundles those files from source, and `tsconfig.ui3.json` type-checks them.

## WearablePreview

`components/WearablePreview` renders the hosted wearable preview in an iframe and talks to it with `postMessage`.

- `protocol.ts` picks the preview origin. The `env` (or `ENV`) query parameter of the page chooses first (`dev`, `stg`, `prod`), then the top-level domain of the host, and any other page gets production. All three origins must stay in the embed list of `packages/creator-hub/main/src/security-restrictions.ts`, or the frame does not load inside Creator Hub.
- The frame URL carries the preview options and ends with `unity=false`.
- The component sends its options once, when the preview reports that it is ready, and again when an option changes.
- `controller.ts` sends requests to the preview (screenshot, metrics, emote playback) and matches each reply to its request. A request fails after 30 seconds without a reply.
- Messages are accepted only from the preview origin and from the window of the frame. Messages are posted to the preview origin only.

## Base styles

The library stylesheet also styled plain elements, and the Inspector's own styles were written on top of those rules. `theme/base.css` keeps the rules the Inspector relies on: border-box sizing, the 14px root size, and the text, heading, link, form control, selection and scrollbar defaults. `index.tsx` imports it before the other stylesheets.

`theme/vars.css` now defines the custom properties that came from the library theme and that Inspector styles still read: `--background`, `--modal`, `--text`, `--secondary-text`, `--text-on-primary`, `--primary` and `--danger`.

Not carried over: the Lato web font request, the Semantic icon fonts, every `.ui` class rule, and rules for elements the Inspector does not render. A bare `legend` element now gets the browser default style.

## When you change this area

- Add a page-wide element default to `theme/base.css`, not to a component stylesheet.
- Add a new preview environment in `protocol.ts` and in the embed list of `security-restrictions.ts` together.
- `pnpm run format` does not check CSS. Check new stylesheets against `.prettierrc` by hand (double quotes).

## Still in the Inspector

Two other UI libraries stay and are separate work:

- `react-contexify` provides the right-click menus.
- `react-dnd` handles drag and drop.
