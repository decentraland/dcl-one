// The HUD host module (`@hud-host`, vite.shared.ts) for the Explorer's own HUD: the shared HUD in
// library/ui/apps/hud reads these through the contract in its hostContract.ts.
import type { HudHost } from '@hud/hostContract'
import { BASE_DOMAIN, DEFAULT_REALM, nativeProfileImage, serviceUrl } from './lib/baseDomain'
import { isNativeHud } from './lib/bootMode'
import { acceptedEntryParams, entryRealm } from './lib/entryParams'
import { PAGE_DIR, publicUrl } from './lib/publicUrl'

/** Site pages follow the domain the Explorer runs against (`?baseDomain=`, decentraland.org by default). */
const siteUrl = (path: string): string => `https://${BASE_DOMAIN}${path}`
/** Items open on the marketplace: this deployment has no Shop page of its own. */
const ownShopItemUrl = (_contract: string, _item: string): string | null => null
/** The Shop opens the web Shop: this HUD's in-world Shop page is the catalogue (ShopPage). */
const IN_WORLD_SHOP = false

export { acceptedEntryParams, BASE_DOMAIN, DEFAULT_REALM, entryRealm, IN_WORLD_SHOP, isNativeHud, nativeProfileImage, ownShopItemUrl, PAGE_DIR, publicUrl, serviceUrl, siteUrl }

const host: HudHost = { acceptedEntryParams, BASE_DOMAIN, DEFAULT_REALM, entryRealm, IN_WORLD_SHOP, isNativeHud, nativeProfileImage, ownShopItemUrl, PAGE_DIR, publicUrl, serviceUrl, siteUrl }
void host
