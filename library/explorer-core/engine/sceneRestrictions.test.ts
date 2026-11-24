import { describe, expect, it } from 'vitest'
import { NO_SCENE_RESTRICTIONS, sceneRestrictionLabels } from './sceneRestrictions'

describe('sceneRestrictionLabels', () => {
  it('lists nothing where the scene restricts nothing', () => {
    expect(sceneRestrictionLabels(NO_SCENE_RESTRICTIONS)).toEqual([])
  })

  it('lists every restriction in one fixed order', () => {
    expect(
      sceneRestrictionLabels({
        nearbyVoiceChatBlocked: true,
        skyboxTimeUiBlocked: true,
        experiencesBlocked: true,
        passportCannotBeOpened: true,
        avatarMovementsBlocked: true,
        avatarHidden: true,
        cameraLocked: true
      })
    ).toEqual([
      'Camera locked',
      'Avatars hidden',
      'Avatar movement disabled',
      'User Options Menu disabled',
      'Experiences disabled',
      'Day/Night controller disabled',
      'Nearby voice disabled'
    ])
  })

  it('lists only what is restricted', () => {
    expect(sceneRestrictionLabels({ ...NO_SCENE_RESTRICTIONS, experiencesBlocked: true, cameraLocked: true })).toEqual(['Camera locked', 'Experiences disabled'])
  })
})
