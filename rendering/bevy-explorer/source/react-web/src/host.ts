// The HUD host module (`@hud-host`, vite.shared.ts) for the Explorer's own HUD: the shared HUD in
// library/ui/apps/hud reads these through the contract in its hostContract.ts.
import type { HudHost } from '@hud/hostContract'
import { nativeProfileImage as proxiedProfileImage } from './lib/baseDomain'

export const nativeProfileImage: HudHost['nativeProfileImage'] = proxiedProfileImage
