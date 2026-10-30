import ReactEcs, { UiEntity } from '@dcl/sdk/react-ecs'
import {
  engine,
  inputSystem,
  InputAction,
  PointerEventType,
  PrimaryPointerInfo
} from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'
import { state, applyBoxSelection } from '../state'
import { computeWorldPositions, shouldMark } from '../world-pos'
import { projectWorldToScreen } from '../camera/camera-projection'
import { liveWorldPos } from './gizmo'
import { pickAtPointer } from './click-select'

const BOX_ADD = Color4.create(0.35, 0.9, 0.45, 1)
const BOX_REMOVE = Color4.create(1, 0.4, 0.35, 1)
const BOX_REPLACE = Color4.create(0.4, 0.7, 1, 1)

const lastMarkers = new Map<string, { x: number; y: number }>()

// In the scene, IaModifier is shift and IaWalk is ctrl.
function clickModifiers(): { shift: boolean; ctrl: boolean } {
  return {
    shift: inputSystem.isPressed(InputAction.IA_MODIFIER),
    ctrl: inputSystem.isPressed(InputAction.IA_WALK)
  }
}

function pointerXY(): { x: number; y: number } | null {
  const p = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenCoordinates
  return p === undefined ? null : { x: p.x, y: p.y }
}

function finishBox(): void {
  const box = state.selectBox
  if (box === null) return
  state.selectBox = null
  // A no-drag tap is a model pick, not an empty box-select.
  if (Math.abs(box.curX - box.startX) < 4 && Math.abs(box.curY - box.startY) < 4) {
    pickAtPointer(box.add, box.remove)
    return
  }
  const minX = Math.min(box.startX, box.curX)
  const maxX = Math.max(box.startX, box.curX)
  const minY = Math.min(box.startY, box.curY)
  const maxY = Math.max(box.startY, box.curY)
  const ids: string[] = []
  for (const [id, p] of lastMarkers) {
    if (p.x >= minX && p.x <= maxX && p.y >= minY && p.y <= maxY) ids.push(id)
  }
  applyBoxSelection(ids, box.add, box.remove)
}

// Drive the drag-box from the live pointer-pressed state rather than UI up/drag
// events: those are missed when the release lands on a marker (which renders on
// top of the surface), wedging the box. onMouseDown still starts it (so we know
// the press began on empty space); this updates and finalizes it.
export function startSelectBox(): void {
  engine.addSystem(() => {
    if (state.playing) {
      state.selectBox = null
      return
    }
    const box = state.selectBox
    if (box === null) {
      // There is no box surface (a blocking UI plane would shadow the
      // engine's world raycast and break click-to-pick) — start the box from
      // raw input instead. DOM panel clicks never reach the canvas, so this
      // only fires for viewport presses.
      if (
        state.activeAction === 'select' &&
        !state.gizmoDragging &&
        state.gizmoHover === null &&
        inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN)
      ) {
        const xy = pointerXY()
        if (xy !== null) {
          const { shift, ctrl } = clickModifiers()
          state.selectBox = { startX: xy.x, startY: xy.y, curX: xy.x, curY: xy.y, add: shift, remove: ctrl }
        }
      }
      return
    }
    if (inputSystem.isPressed(InputAction.IA_POINTER)) {
      const xy = pointerXY()
      if (xy !== null) {
        box.curX = xy.x
        box.curY = xy.y
      }
      // Track the modifier live so add/remove reflect the state at release, not
      // at press (and the rubber-band colour updates as you hold shift/ctrl).
      const { shift, ctrl } = clickModifiers()
      box.add = shift
      box.remove = ctrl
    } else {
      finishBox()
    }
  })
}

function selectionBox(): ReactEcs.JSX.Element | [] {
  const b = state.selectBox
  if (b === null) return []
  const color = b.remove ? BOX_REMOVE : b.add ? BOX_ADD : BOX_REPLACE
  return (
    <UiEntity
      key="selbox"
      uiTransform={{
        positionType: 'absolute',
        position: { left: Math.min(b.startX, b.curX), top: Math.min(b.startY, b.curY) },
        width: Math.abs(b.curX - b.startX),
        height: Math.abs(b.curY - b.startY),
        borderWidth: 1,
        borderColor: color,
        pointerFilter: 'none'
      }}
      uiBackground={{ color: { ...color, a: 0.15 } }}
    />
  )
}

export function overlayUi(): ReactEcs.JSX.Element | null {
  if (state.status !== 'ready' || state.playing) return null

  const worldPositions = computeWorldPositions(state.snapshot)
  if (worldPositions === null) return null

  lastMarkers.clear()
  for (const [id, world] of worldPositions) {
    if (!shouldMark(state.snapshot, id)) continue
    const screen = projectWorldToScreen(liveWorldPos(id, world))
    if (screen === null || !screen.onScreen) continue
    // box-select hit-testing needs every candidate position
    lastMarkers.set(id, { x: screen.left, y: screen.top })
  }

  return (
    <UiEntity
      uiTransform={{
        width: '100%',
        height: '100%',
        positionType: 'absolute',
        position: { top: 0, left: 0 },
        pointerFilter: 'none'
      }}
    >
      {state.activeAction === 'select' ? selectionBox() : []}
    </UiEntity>
  )
}

// Root UI renderer for the scene (set via ReactEcsRenderer in index.ts). The
// host-page React app (packages/ui) is the editor's only panel UI; the scene
// renders ONLY the viewport layer it must own because it needs engine camera
// projection: the select-tool drag-box.
// The gizmo handles and parent/child relation lines are NO LONGER composited here
// as videoTexture UI backgrounds — that secondary-camera composite blacked the
// whole frame in wasm (PLACEMENT_BLACK). They now render as ordinary main-pass
// (layer-0) 3D meshes drawn by the primary camera; the selection outline uses the
// engine post-process. So this renderer owns only the DOM-projected select UI.
export function inspectorUi(): ReactEcs.JSX.Element {
  return (
    <UiEntity
      uiTransform={{
        width: '100%',
        height: '100%',
        positionType: 'absolute',
        position: { top: 0, left: 0 },
        pointerFilter: 'none'
      }}
    >
      {overlayUi() ?? []}
    </UiEntity>
  )
}
