import {
  engine,
  Transform,
  MeshRenderer,
  Material,
  VisibilityComponent,
  type Entity
} from '@dcl/sdk/ecs'
import { CameraLayers } from '../camera-layers'
import { Vector3, Quaternion, Color4 } from '@dcl/sdk/math'
import { state, parentOf, buildForest } from '../state'
import { computeWorldPositions } from '../world-pos'
import { liveWorldPos } from './gizmo'


const TO_PARENT = Color4.create(0.2, 0.4, 1, 1)
const TO_CHILD = Color4.create(0.7, 0.95, 0.15, 1)
const BOTH = Color4.create(0.2, 0.85, 0.9, 1)
const MAX_LINES = 192

let relRoot: Entity | null = null
const lines: Entity[] = []
const lineShown: boolean[] = []

export function setupRelations(): void {
  if (relRoot !== null) return

  const root = engine.addEntity()
  Transform.create(root)
  CameraLayers.create(root, { layers: [8] })
  for (let i = 0; i < MAX_LINES; i++) {
    const e = engine.addEntity()
    Transform.create(e, { parent: root })
    MeshRenderer.setBox(e)
    VisibilityComponent.create(e, { visible: false })
    lines.push(e)
    lineShown.push(false)
  }
  relRoot = root

  engine.addSystem(updateRelations)
}

function edgeStyle(p: string, c: string): { color: Color4; alpha: number } {
  if (state.activeEntity === c) return { color: TO_PARENT, alpha: 1 }
  if (state.activeEntity === p) return { color: TO_CHILD, alpha: 1 }
  if (state.selected.has(p) && state.selected.has(c)) return { color: BOTH, alpha: 0.5 }
  if (state.selected.has(c)) return { color: TO_PARENT, alpha: 0.5 }
  return { color: TO_CHILD, alpha: 0.5 }
}

function setLine(
  e: Entity,
  a: Vector3,
  b: Vector3,
  cam: Vector3,
  color: Color4,
  alpha: number
): void {
  const dir = Vector3.subtract(b, a)
  const len = Vector3.length(dir)
  const mid = Vector3.scale(Vector3.add(a, b), 0.5)
  const t = Transform.getMutable(e)
  t.position = mid
  t.rotation = Quaternion.fromToRotation(Vector3.Up(), dir)
  const thick = Math.max(0.01, Vector3.distance(mid, cam) * 0.0045)
  t.scale = Vector3.create(thick, len, thick)
  const mx = Math.max(color.r, color.g, color.b, 0.001)
  const er = Math.pow(color.r / mx, 2.2)
  const eg = Math.pow(color.g / mx, 2.2)
  const eb = Math.pow(color.b / mx, 2.2)
  Material.setPbrMaterial(e, {
    albedoColor: { r: er, g: eg, b: eb, a: alpha },
    emissiveColor: { r: er, g: eg, b: eb },
    emissiveIntensity: 0.8,
    roughness: 1
  })
}

function showLine(i: number, on: boolean): void {
  if (lineShown[i] === on) return
  lineShown[i] = on
  VisibilityComponent.getMutable(lines[i]).visible = on
}

function updateRelations(): void {
  if (relRoot === null) return

  if (state.playing || !state.showLinks || state.selected.size === 0) {
    for (let i = 0; i < lines.length; i++) showLine(i, false)
    return
  }

  const camT = Transform.getOrNull(engine.CameraEntity)
  if (camT === null) return

  const world = computeWorldPositions(state.snapshot)
  if (world === null) return

  const livePos = (id: string): Vector3 => liveWorldPos(id, world.get(id) as Vector3)

  const forest = buildForest(state.snapshot)
  const seen = new Set<string>()
  const edges: Array<{ p: string; c: string }> = []
  const add = (p: string, c: string): void => {
    if (!world.has(p) || !world.has(c)) return
    const key = `${p}>${c}`
    if (seen.has(key)) return
    seen.add(key)
    edges.push({ p, c })
  }
  for (const s of state.selected) {
    const par = parentOf(state.snapshot, s)
    if (par !== null && par !== '0') add(par, s)
    for (const child of forest.children.get(s) ?? []) add(s, child)
  }

  let n = 0
  for (const { p, c } of edges) {
    if (n >= lines.length) break
    const a = livePos(p)
    const b = livePos(c)
    const { color, alpha } = edgeStyle(p, c)
    setLine(lines[n], a, b, camT.position, color, alpha)
    showLine(n, true)
    n++
  }
  for (let i = n; i < lines.length; i++) showLine(i, false)
}
