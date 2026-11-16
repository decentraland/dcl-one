// The HUD host module (`@hud-host`, vite.shared.ts) for the Explorer's own HUD: the shared HUD in
// library/ui/apps/hud reads these through the contract in its hostContract.ts.
import type { HudHost, UnattendedRun } from '@explorer-core/hostContract'
import { BASE_DOMAIN, DEFAULT_REALM, nativeProfileImage, serviceUrl } from './lib/baseDomain'
import { BENCHMARK_PARAMS, benchmarkSwitchesHonoured, bootMode, isNativeHud } from './lib/bootMode'
import type { AutopilotDriver } from './engine/autopilot'
import { acceptedEntryParams, entryRealm, unrecognisedEntryParams } from './lib/entryParams'
import { untrustedLaunchParams } from './lib/launchGate'
import { PAGE_DIR, publicUrl } from './lib/publicUrl'
import { engineLaunch, seedBridgeSession } from './lib/engineLaunch'

/** Site pages follow the domain the Explorer runs against (`?baseDomain=`, decentraland.org by default). */
const siteUrl = (path: string): string => `https://${BASE_DOMAIN}${path}`
/** Items open on the marketplace: this deployment has no Shop page of its own. */
const ownShopItemUrl = (_contract: string, _item: string): string | null => null
/** The Shop opens the web Shop: this HUD's in-world Shop page is the catalogue (ShopPage). */
const IN_WORLD_SHOP = false
/** No Donate: this HUD's page holds no wallet that can sign a tip. */
const donations: HudHost['donations'] = null
/** The benchmark autopilot (?autopilot, where lib/bootMode.ts honours it): it signs in with the
 *  machine's local guest and nothing else, lands at Genesis 0,0, and a sign-in that fails ends the
 *  run through the engine (non-zero, no CSV). */
const unattendedRun = (): UnattendedRun | null =>
  bootMode().autoLogin === 'unattended'
    ? {
        destination: { kind: 'parcel', x: 0, y: 0 },
        login: (d) => (d as AutopilotDriver).loginLocalGuest?.() ?? d.loginGuest(),
        loginFailed: (d, reason) =>
          void (d as AutopilotDriver)
            .autopilotFailed?.(reason)
            ?.catch((e: unknown) => console.error('[autopilot] the engine was not told the sign-in failed:', e))
      }
    : null

/** boot.js sets it when the engine ended its own run (an `AppExit`, the benchmark autopilot). */
const engineExited = (): boolean => (window as { __engineExited?: boolean }).__engineExited === true

/** The benchmark's switches, where this page honours them (lib/bootMode.ts). */
const hostEntryParams = (): string[] => (benchmarkSwitchesHonoured() ? BENCHMARK_PARAMS : [])

/** The commit the HUD was built from (vite.config.ts `define`). */
const BUILD_VERSION: string | null = (typeof __HUD_BUILD__ !== 'undefined' && __HUD_BUILD__) || null

/** The gate's Exit leaves for decentraland.org, whatever `?baseDomain=` the link carries. */
const exitUrl = (): string => 'https://decentraland.org'

export { acceptedEntryParams, BUILD_VERSION, exitUrl, hostEntryParams, unrecognisedEntryParams, untrustedLaunchParams, BASE_DOMAIN, bootMode, engineExited, engineLaunch, seedBridgeSession, DEFAULT_REALM, entryRealm, IN_WORLD_SHOP, donations, isNativeHud, nativeProfileImage, ownShopItemUrl, PAGE_DIR, publicUrl, serviceUrl, siteUrl, unattendedRun }

const host: HudHost = { acceptedEntryParams, BUILD_VERSION, exitUrl, hostEntryParams, unrecognisedEntryParams, untrustedLaunchParams, BASE_DOMAIN, bootMode, engineExited, engineLaunch, seedBridgeSession, DEFAULT_REALM, entryRealm, IN_WORLD_SHOP, donations, isNativeHud, nativeProfileImage, ownShopItemUrl, PAGE_DIR, publicUrl, serviceUrl, siteUrl, unattendedRun }
void host
