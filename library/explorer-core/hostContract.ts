// What each HUD host supplies through the `@hud-host` alias: the behaviours that differ between the
// Explorer's own HUD (rendering/bevy-explorer/source/react-web) and the Sites /play host. Each host
// maps `@hud-host` to a module implementing this contract (react-web/src/host.ts,
// apps/ui3/src/play/host.ts), so the shared HUD code stays one copy.
import type { LoginDriver } from './engine/driver'

/** A parameter an entry url may carry, as the entry-params dialog lists it. */
export interface AcceptedParam {
  name: string
  doc: string
  /** who consumes it: the engine (via the table) or this page */
  reader: 'engine' | 'page'
}

/** How this page boots, read from its entry url. */
export interface BootMode {
  /** explicit ?systemScene= override; null = the default bridge scene drives */
  systemScene: string | null
  /** render only the engine — no HUD chrome, no sign-in/loading overlays */
  hideHud: boolean
  /** skip the sign-in screen: auto guest-login, no React login at all (the ui scene owns it), or the
   *  host's unattended run (`unattendedRun`) */
  autoLogin: 'guest' | 'scene-owned' | 'unattended' | null
}

/** A run with no one at the page (react-web's benchmark autopilot): the session skips every step a
 *  person takes and leaves its sign-in, and that sign-in's failure, to the run. */
export interface UnattendedRun {
  /** Where the run goes, with no pre-flight: an unreachable realm fails in the engine. */
  destination: { kind: 'parcel'; x: number; y: number }
  /** Sign in with the run's own identity. */
  login(driver: LoginDriver): Promise<void>
  /** The sign-in failed: the run ends, and nothing returns to a sign-in screen. */
  loginFailed(driver: LoginDriver, reason: string): void
}

export interface HudHost {
  /** The URL this host loads a profile image from: the native HUD routes profile images through its
   *  own proxy, the web hosts load them as given. */
  nativeProfileImage(raw: string | undefined): string | undefined
  /** A page of the Decentraland site (`/terms/`, `/marketplace`, ...) for the deployment this HUD
   *  runs against. */
  siteUrl(path: string): string
  /** Where this host's deployment serves the service named in the service table. */
  serviceUrl(name: string): string
  /** The domain this HUD's deployment runs on. */
  BASE_DOMAIN: string
  /** The realm a session joins when the entry url names none. */
  DEFAULT_REALM: string
  /** Whether this page is the native (CEF) HUD. */
  isNativeHud(): boolean
  /** The directory the HUD's page is served from. */
  PAGE_DIR: string
  /** A public-dir asset of this host, resolved for runtime use. */
  publicUrl(path: string): string
  /** The entry parameters this host's page advertises. */
  acceptedEntryParams(): AcceptedParam[]
  /** The realm the entry url asks for, or null. */
  entryRealm(q: URLSearchParams): string | null
  /** This deployment's own Shop page for a collection item, or null when the marketplace shows it. */
  ownShopItemUrl(contract: string, item: string): string | null
  /** Whether this host runs the in-world Shop page (otherwise the Shop opens the web Shop). */
  IN_WORLD_SHOP: boolean
  /** Whether the engine ended its own run (react-web's benchmark autopilot): the bridge scene went
   *  with it, so its silence is no fault. */
  engineExited(): boolean
  /** How this page boots. */
  bootMode(): BootMode
  /** This boot's unattended run, when it is one (`bootMode().autoLogin === 'unattended'`), else null. */
  unattendedRun(): UnattendedRun | null
}
