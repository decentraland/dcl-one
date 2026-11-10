// The `@hud-host` module the library's own HUD tests run against: a web host on decentraland.org,
// with images as given and no Shop page of its own.
import type { AcceptedParam, BootMode, HudHost } from '@explorer-core/hostContract'

export const BASE_DOMAIN = 'decentraland.org'
export const nativeProfileImage: HudHost['nativeProfileImage'] = (raw) => raw
export const siteUrl: HudHost['siteUrl'] = (path) => `https://${BASE_DOMAIN}${path}`
export const serviceUrl: HudHost['serviceUrl'] = (name) => `https://${name}.${BASE_DOMAIN}`
export const DEFAULT_REALM = `https://realm-provider-ea.${BASE_DOMAIN}/main`
export const isNativeHud: HudHost['isNativeHud'] = () => false
export const PAGE_DIR = 'http://localhost/'
export const publicUrl: HudHost['publicUrl'] = (path) => new URL(path, PAGE_DIR).href
export const acceptedEntryParams = (): AcceptedParam[] => []
export const entryRealm: HudHost['entryRealm'] = (q) => q.get('realm') || null
export const ownShopItemUrl: HudHost['ownShopItemUrl'] = () => null
export const IN_WORLD_SHOP = false
export const bootMode = (): BootMode => ({ systemScene: null, hideHud: false, autoLogin: null })
export const unattendedRun: HudHost['unattendedRun'] = () => null
export const engineExited: HudHost['engineExited'] = () => false
