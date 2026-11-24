// The HUD host module (`@hud-host`) for the Sites /play host: the shared HUD in library/ui/apps/hud
// reads these through the contract in its hostContract.ts.
import type { HudHost } from '@explorer-core/hostContract'
import { BASE_DOMAIN, checkoutSiteUrl, DEFAULT_REALM, gatewaySignedPath, GATEWAY, ONE_GATEWAY_HOST, serviceUrl, siteUrl } from './lib/baseDomain'
import { gatewayProfileImage } from '@explorer-core/lib/gateway'
import { bootMode, isNativeHud } from './lib/bootMode'
import { acceptedEntryParams, entryRealm, unrecognisedEntryParams } from './lib/entryParams'
import { untrustedLaunchParams } from './lib/launchGate'
import { PAGE_DIR, publicUrl } from './lib/publicUrl'
import { engineLaunch, seedBridgeSession } from './lib/engineLaunch'
import { IN_WORLD_SHOP } from '@dcl/one-ui/apps/shop/entry'
import { donations } from './lib/donations'
import { gifting } from './lib/gifting'

/** Sites loads profile images as given, or through the entry url's ?gateway= as the engine does. */
const nativeProfileImage = (raw: string | undefined): string | undefined =>
  GATEWAY == null || raw == null ? raw : (gatewayProfileImage(GATEWAY, raw, BASE_DOMAIN) ?? raw)
/** dcl-one's Shop serves a collection item at /shop/item/<contract>/<item>; the Foundation's
 *  deployments (`?baseDomain=decentraland.org`) keep the marketplace's own page. */
const ownShopItemUrl = (contract: string, item: string): string | null =>
  BASE_DOMAIN === ONE_GATEWAY_HOST ? siteUrl(`/shop/item/${contract}/${item}`) : null

/** /play has no benchmark run. */
const unattendedRun: HudHost['unattendedRun'] = () => null

/** /play's engine never ends its own run: it has no benchmark. */
const engineExited = (): boolean => false

/** /play acts on no params of its own beyond the shared ones. */
const hostEntryParams = (): string[] => []

/** The commit Sites built /play from (apps/sites/vite.config.ts `define`), else the image's `COMMIT_HASH`, null without either. */
declare const __PLAY_BUILD__: string | undefined
const BUILD_VERSION: string | null = (typeof __PLAY_BUILD__ !== 'undefined' && __PLAY_BUILD__)
  || window.__DCL_PUBLIC__?.commitHash?.replace(/^([0-9a-f]{9})[0-9a-f]{31}/, '$1') || null

/** The gate's Exit leaves for the Sites page this HUD is served from, whatever `?baseDomain=` the link
 *  carries. */
const exitUrl = (): string => `${window.location.origin}/`

/** The /play shell asks before the page goes while a session runs, when the player turns it on (shell/mount.ts). */
const CAN_ASK_BEFORE_LEAVING = true

const siteRevision: HudHost['siteRevision'] = () =>
  typeof window.__DCL_PUBLIC__ === 'object' ? window.__DCL_PUBLIC__.commitHash ?? '' : null

export { acceptedEntryParams, BUILD_VERSION, exitUrl, hostEntryParams, unrecognisedEntryParams, untrustedLaunchParams, BASE_DOMAIN, bootMode, engineExited, engineLaunch, seedBridgeSession, DEFAULT_REALM, entryRealm, IN_WORLD_SHOP, CAN_ASK_BEFORE_LEAVING, donations, gifting, siteRevision, isNativeHud, nativeProfileImage, ownShopItemUrl, PAGE_DIR, publicUrl, serviceUrl, gatewaySignedPath, siteUrl, checkoutSiteUrl, unattendedRun }

const host: HudHost = { acceptedEntryParams, BUILD_VERSION, exitUrl, hostEntryParams, unrecognisedEntryParams, untrustedLaunchParams, BASE_DOMAIN, bootMode, engineExited, engineLaunch, seedBridgeSession, DEFAULT_REALM, entryRealm, IN_WORLD_SHOP, CAN_ASK_BEFORE_LEAVING, donations, gifting, siteRevision, isNativeHud, nativeProfileImage, ownShopItemUrl, PAGE_DIR, publicUrl, serviceUrl, gatewaySignedPath, siteUrl, checkoutSiteUrl, unattendedRun }
void host
