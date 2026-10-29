import assert from 'node:assert/strict'
import { test } from 'node:test'
import { Vector3, Quaternion } from '@dcl/sdk/math'
import { state, revealGizmoForSelection, setActiveAction, type Snapshot } from '../src/state'
import { worldTransformOf, worldToLocalPosition, worldToLocalRotation, computeWorldPositions, rootLocalForWorld } from '../src/world-pos'
import { axisDragPlaneNormal, constrainTranslation, rayPlaneIntersect, angleOnPlane, axisVec } from '../src/viewport/gizmo-pick'

const transform = (parent = 0, x = 0, z = 0) => ({ parent, position: { x, y: 0, z }, rotation: { x: 0, y: 0, z: 0, w: 1 }, scale: { x: 1, y: 1, z: 1 } })
const projectScene = (): Snapshot => ({ '0': {}, '512': { Transform: transform(0, 2, 2) }, '513': { Transform: transform(512, 1, 0) } })
const near = (a: { x: number; y: number; z: number }, b: { x: number; y: number; z: number }) => {
  for (const k of ['x', 'y', 'z'] as const) assert.ok(Math.abs(a[k] - b[k]) < 1e-6, `${k}: ${a[k]} != ${b[k]}`)
}

// Module state is shared by the tests below, so the boot defaults are checked first.
test('the scene boots with the move tool so the gizmo shows on first selection', () => {
  assert.equal(state.activeAction, 'translate')
  assert.equal(state.lastTool, 'translate')
})

test('a project scene without the editor origin entity still has world transforms', () => {
  const snapshot = projectScene()
  near(worldTransformOf(snapshot, '512')!.position, { x: 2, y: 0, z: 2 })
  near(worldTransformOf(snapshot, '513')!.position, { x: 3, y: 0, z: 2 })
  near(worldToLocalPosition(snapshot, '513', Vector3.create(5, 0, 2))!, { x: 3, y: 0, z: 0 })
  assert.deepEqual(worldToLocalRotation(snapshot, '513', Quaternion.Identity()), Quaternion.Identity())
  near(computeWorldPositions(snapshot)!.get('513')!, { x: 3, y: 0, z: 2 })
  near(rootLocalForWorld(snapshot, Vector3.create(1, 0, 1))!, { x: 1, y: 0, z: 1 })
  assert.equal(worldTransformOf(snapshot, '999'), null)
})

test('the editor origin entity still offsets world space when present', () => {
  const snapshot = { ...projectScene(), '5': { Transform: transform(0, 16, 16) } }
  near(worldTransformOf(snapshot, '512')!.position, { x: -14, y: 0, z: -14 })
  near(worldToLocalPosition(snapshot, '512', Vector3.create(-14, 0, -14))!, { x: 2, y: 0, z: 2 })
})

test('selecting an entity under the select tool reveals the last transform tool', () => {
  setActiveAction('rotate')
  setActiveAction('select')
  assert.equal(state.activeAction, 'select')
  state.activeEntity = null
  revealGizmoForSelection()
  assert.equal(state.activeAction, 'select')
  state.activeEntity = '512'
  revealGizmoForSelection()
  assert.equal(state.activeAction, 'rotate')
  setActiveAction('select')
  revealGizmoForSelection()
  assert.equal(state.activeAction, 'select', 'an unchanged selection keeps the select tool')
  state.activeEntity = '513'
  revealGizmoForSelection()
  assert.equal(state.activeAction, 'rotate')
})

test('an axis drag follows only that axis, on a plane facing the camera', () => {
  const camPos = Vector3.create(3, 6, -10)
  const center = Vector3.create(4, 0, 4)
  const camForward = Vector3.normalize(Vector3.subtract(center, camPos))
  const axis = axisVec('x')
  const normal = axisDragPlaneNormal(axis, camForward)
  assert.ok(Math.abs(Vector3.dot(normal, axis)) < 1e-6, 'the drag plane contains the axis')
  assert.ok(Math.abs(Vector3.dot(normal, camForward)) > 0.5, 'the drag plane faces the camera')

  const rayTo = (p: Vector3) => Vector3.normalize(Vector3.subtract(p, camPos))
  const startHit = rayPlaneIntersect(camPos, rayTo(center), center, normal)!
  near(startHit, center)
  const across = Vector3.normalize(Vector3.cross(normal, axis))
  const target = Vector3.add(center, Vector3.add(Vector3.scale(axis, 2.5), Vector3.scale(across, 1.5)))
  const hit = rayPlaneIntersect(camPos, rayTo(target), center, normal)!
  near(hit, target)
  const delta = constrainTranslation({ op: 'translate-axis', axis: 'x' }, Vector3.subtract(hit, startHit))
  near(delta, { x: 2.5, y: 0, z: 0 })

  const planar = constrainTranslation({ op: 'translate-plane', normal: 'y' }, Vector3.create(1, 7, -2))
  near(planar, { x: 1, y: 0, z: -2 })
})

test('a rotate drag measures the angle swept around the ring axis', () => {
  const center = Vector3.create(4, 0, 4)
  const normal = axisVec('y')
  const a0 = angleOnPlane(Vector3.add(center, Vector3.create(1, 0, 0)), center, normal)
  const a1 = angleOnPlane(Vector3.add(center, Vector3.create(0, 0, 1)), center, normal)
  const swept = Math.atan2(Math.sin(a1 - a0), Math.cos(a1 - a0))
  assert.ok(Math.abs(Math.abs(swept) - Math.PI / 2) < 1e-6, `swept ${swept}`)
})
