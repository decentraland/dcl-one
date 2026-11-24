// The lines of the minimap's scene restrictions list, in the order it always shows them.
import type { SceneRestrictions } from './generated'

export const NO_SCENE_RESTRICTIONS: SceneRestrictions = {
  cameraLocked: false,
  avatarHidden: false,
  avatarMovementsBlocked: false,
  passportCannotBeOpened: false,
  experiencesBlocked: false,
  skyboxTimeUiBlocked: false,
  nearbyVoiceChatBlocked: false
}

const LABELS: ReadonlyArray<[keyof SceneRestrictions, string]> = [
  ['cameraLocked', 'Camera locked'],
  ['avatarHidden', 'Avatars hidden'],
  ['avatarMovementsBlocked', 'Avatar movement disabled'],
  ['passportCannotBeOpened', 'User Options Menu disabled'],
  ['experiencesBlocked', 'Experiences disabled'],
  ['skyboxTimeUiBlocked', 'Day/Night controller disabled'],
  ['nearbyVoiceChatBlocked', 'Nearby voice disabled']
]

export function sceneRestrictionLabels(state: SceneRestrictions): string[] {
  return LABELS.filter(([key]) => state[key]).map(([, label]) => label)
}
