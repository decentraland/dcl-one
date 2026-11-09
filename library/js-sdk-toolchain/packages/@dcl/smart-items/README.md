# @dcl/smart-items

The runtime of the smart items a scene authors in the Creator Hub: the `smart-items::*` components
(actions, triggers, states, counters, admin tools, video screens, scripts) and the systems that run
them, plus the admin toolkit UI.

The scene build boots it from `@dcl/smart-items` (`initSmartItems`) when `main.composite` carries a
smart-item component, and resolves the imports of the deprecated `@dcl/asset-packs` package to it:
`@dcl/asset-packs` and `@dcl/asset-packs/dist/scene-entrypoint` to the root, `@dcl/asset-packs/dist/<module>`
to `@dcl/smart-items/<module>`.

Composites saved under the former `asset-packs::` component names are renamed in place
(`renameLegacyComposite` in `src/legacy.ts`), and the component numbers listed by
`core-schema::Sync-Components` are remapped.

```ts
import { getTriggerEvents, getActionEvents } from '@dcl/smart-items/events'
import { TriggerType } from '@dcl/smart-items'
```
