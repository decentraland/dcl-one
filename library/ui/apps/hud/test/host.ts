// The `@hud-host` module the library's own HUD tests run against: a web host, with images as given.
import type { HudHost } from '../hostContract'

export const nativeProfileImage: HudHost['nativeProfileImage'] = (raw) => raw
