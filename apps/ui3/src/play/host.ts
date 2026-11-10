// The HUD host module (`@hud-host`) for the Sites /play host: the shared HUD in library/ui/apps/hud
// reads these through the contract in its hostContract.ts.
import type { HudHost } from '@explorer-core/hostContract'
import { BASE_DOMAIN, DEFAULT_REALM, ONE_GATEWAY_HOST, serviceUrl, siteUrl } from './lib/baseDomain'
import { bootMode, isNativeHud } from './lib/bootMode'
import { acceptedEntryParams, entryRealm } from './lib/entryParams'
import { PAGE_DIR, publicUrl } from './lib/publicUrl'
import { IN_WORLD_SHOP } from './features/shop/entry'

/** Sites loads profile images as given: it has no native image proxy. */
const nativeProfileImage = (raw: string | undefined): string | undefined => raw
/** dcl-one's Shop serves a collection item at /shop/item/<contract>/<item>; the Foundation's
 *  deployments (`?baseDomain=decentraland.org`) keep the marketplace's own page. */
const ownShopItemUrl = (contract: string, item: string): string | null =>
  BASE_DOMAIN === ONE_GATEWAY_HOST ? siteUrl(`/shop/item/${contract}/${item}`) : null

/** /play has no benchmark run. */
const unattendedRun: HudHost['unattendedRun'] = () => null

export { acceptedEntryParams, BASE_DOMAIN, bootMode, DEFAULT_REALM, entryRealm, IN_WORLD_SHOP, isNativeHud, nativeProfileImage, ownShopItemUrl, PAGE_DIR, publicUrl, serviceUrl, siteUrl, unattendedRun }

const host: HudHost = { acceptedEntryParams, BASE_DOMAIN, bootMode, DEFAULT_REALM, entryRealm, IN_WORLD_SHOP, isNativeHud, nativeProfileImage, ownShopItemUrl, PAGE_DIR, publicUrl, serviceUrl, siteUrl, unattendedRun }
void host
