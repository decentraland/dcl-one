// What each HUD host supplies through the `@hud-host` alias: the few behaviours that differ between the
// Explorer's own HUD (rendering/bevy-explorer/source/react-web) and the Sites /play adaptation. Each host
// maps `@hud-host` to a module that implements this contract (react-web/src/host.ts,
// apps/ui3/src/play/host.ts), so the shared HUD code stays one copy.
export interface HudHost {
  /** The URL this host loads a profile image from: the native HUD routes profile images through its
   *  own proxy, the web hosts load them as given. */
  nativeProfileImage(raw: string | undefined): string | undefined
}
