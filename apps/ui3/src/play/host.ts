// The HUD host module (`@hud-host`) for the Sites /play adaptation: the shared HUD in
// library/ui/apps/hud reads these through the contract in its hostContract.ts.
import type { HudHost } from '@hud/hostContract'

/** Sites loads profile images as given: it has no native image proxy. */
export const nativeProfileImage: HudHost['nativeProfileImage'] = (raw) => raw
