// The local user's ban from the scene or world they stand in, as the HUD words it: the name of the
// minimap's mark, and the notice shown once per ban and on the mark's hover.

import type { SceneBanScope } from '@explorer-core/engine/generated'

export const SCENE_BAN_LABEL: Record<SceneBanScope, string> = {
  scene: 'Banned from this scene',
  world: 'Banned from this world'
}

export const SCENE_BAN_NOTICE: Record<SceneBanScope, string> = {
  scene: 'Scene access restricted: you are banned from this scene. It stays unloaded, and players here cannot see you or your Nearby chat.',
  world: 'World access restricted: you are banned from this world. Its scenes stay unloaded, and players here cannot see you or your Nearby chat.'
}

/** How long the notice stays up by itself, as unity-explorer's ban tooltip does. */
export const SCENE_BAN_NOTICE_MS = 10_000

/** The notice's toast key: an admission takes it down. */
export const SCENE_BAN_TOAST = 'scene-ban'
