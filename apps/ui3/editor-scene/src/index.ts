import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { engine, Transform, AvatarModifierArea, AvatarModifierType } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'
import { startInspector } from './inspector/session'
import { startCameraProjection } from './camera/camera-projection'
import { setupGizmo } from './viewport/gizmo'
import { setupRelations } from './viewport/relations'
import { setupCamera } from './camera/free-cam'
import { startSelectBox, inspectorUi } from './viewport/overlay'
import { startSystemActions } from './system-actions'
import { startPageUiBridge } from './page-ui'
import { startSelectionHighlight, startGizmoPick, setupMeshSelect } from './viewport/click-select'
import { setupEditorAmbiance } from './viewport/ambiance'
import { state } from './state'

function hideAvatarsInEditMode(): void {
  const area = engine.addEntity()
  Transform.create(area, { position: Vector3.create(8, 0, 8) })
  let hidden: boolean | undefined
  engine.addSystem(() => {
    if (hidden === !state.playing) return
    hidden = !state.playing
    if (!hidden) AvatarModifierArea.deleteFrom(area)
    else AvatarModifierArea.createOrReplace(area, {
      area: Vector3.create(4096, 4096, 4096),
      modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
      excludeIds: []
    })
  })
}

export function main(): void {
  const _log = console.log
  console.log = (...args: any[]) => {
    _log('[Component Inspector]', ...args)
  }

  hideAvatarsInEditMode()
  startCameraProjection()
  setupGizmo()
  setupRelations()
  setupCamera()
  startSelectBox()
  startSystemActions()
  startPageUiBridge()
  startSelectionHighlight()
  setupMeshSelect()
  startGizmoPick()
  setupEditorAmbiance()
  ReactEcsRenderer.setUiRenderer(inspectorUi)

  startInspector().catch((e) => {
    console.error('fatal error during inspector init')
    console.error(e)
  })
}
