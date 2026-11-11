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
export const seedBridgeSession: HudHost['seedBridgeSession'] = () => {}
export const engineLaunch: HudHost['engineLaunch'] = () => ({ options: {} as ReturnType<HudHost['engineLaunch']>['options'], engineDir: 'http://localhost/engine' })
export const untrustedLaunchParams: HudHost['untrustedLaunchParams'] = () => []
export const unrecognisedEntryParams: HudHost['unrecognisedEntryParams'] = () => []
export const hostEntryParams: HudHost['hostEntryParams'] = () => []
export const exitUrl: HudHost['exitUrl'] = () => 'https://decentraland.org'

// Fails the type check when the contract gains a member this stub lacks, as each real host does.
const host: HudHost = { acceptedEntryParams, exitUrl, hostEntryParams, unrecognisedEntryParams, untrustedLaunchParams, BASE_DOMAIN, bootMode, engineExited, engineLaunch, seedBridgeSession, DEFAULT_REALM, entryRealm, IN_WORLD_SHOP, isNativeHud, nativeProfileImage, ownShopItemUrl, PAGE_DIR, publicUrl, serviceUrl, siteUrl, unattendedRun }
void host
