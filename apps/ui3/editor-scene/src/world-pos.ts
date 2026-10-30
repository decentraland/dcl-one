import { Vector3, Quaternion } from '@dcl/sdk/math'
import { type Snapshot } from './state'
import { NAME_COMPONENT } from './custom-components'
import { rotateVec3ByQuat } from './camera/perspective-to-screen'

type Trs = { pos: Vector3; rot: Quaternion; scale: Vector3 }

type TransformValue = {
  position?: { x: number; y: number; z: number }
  rotation?: { x: number; y: number; z: number; w: number }
  scale?: { x: number; y: number; z: number }
  parent?: number
}

function readTransform(snapshot: Snapshot, id: string): TransformValue {
  return (snapshot[id]?.Transform as TransformValue | undefined) ?? {}
}

function localTrs(t: TransformValue): Trs {
  const p = t.position
  const r = t.rotation
  const s = t.scale
  return {
    pos: p ? Vector3.create(p.x, p.y, p.z) : Vector3.Zero(),
    rot: r ? Quaternion.create(r.x, r.y, r.z, r.w) : Quaternion.Identity(),
    scale: s ? Vector3.create(s.x, s.y, s.z) : Vector3.One()
  }
}

function composed(
  snapshot: Snapshot,
  id: string,
  cache: Map<string, Trs>,
  visiting: Set<string>
): Trs {
  const cached = cache.get(id)
  if (cached !== undefined) return cached

  const t = readTransform(snapshot, id)
  const local = localTrs(t)
  const parent = t.parent ?? 0

  let result: Trs
  if (parent === 0 || visiting.has(String(parent)) || !(String(parent) in snapshot)) {
    result = local
  } else {
    visiting.add(id)
    const P = composed(snapshot, String(parent), cache, visiting)
    visiting.delete(id)
    const scaled = Vector3.multiply(local.pos, P.scale)
    const rotated = rotateVec3ByQuat(scaled, P.rot)
    result = {
      pos: Vector3.add(P.pos, rotated),
      rot: Quaternion.multiply(P.rot, local.rot),
      scale: Vector3.multiply(P.scale, local.scale)
    }
  }

  cache.set(id, result)
  return result
}

/** Entity '5' is the dcl-editor scene-root offset; a project scene authored
 *  in the Creator Hub has none, so world space is then the scene's own. */
function originOf(snapshot: Snapshot, cache: Map<string, Trs>): Vector3 {
  return '5' in snapshot ? composed(snapshot, '5', cache, new Set()).pos : Vector3.Zero()
}

export function computeWorldPositions(
  snapshot: Snapshot
): Map<string, Vector3> | null {
  const cache = new Map<string, Trs>()
  const origin = originOf(snapshot, cache)

  const out = new Map<string, Vector3>()
  for (const id of Object.keys(snapshot)) {
    const pos = composed(snapshot, id, cache, new Set()).pos
    out.set(id, Vector3.subtract(pos, origin))
  }
  return out
}

export function worldTransformOf(
  snapshot: Snapshot,
  id: string
): { position: Vector3; rotation: Quaternion } | null {
  if (!(id in snapshot)) return null
  const cache = new Map<string, Trs>()
  const origin = originOf(snapshot, cache)
  const trs = composed(snapshot, id, cache, new Set())
  return { position: Vector3.subtract(trs.pos, origin), rotation: trs.rot }
}

export function worldPositionOf(snapshot: Snapshot, id: string): Vector3 | null {
  return worldTransformOf(snapshot, id)?.position ?? null
}

export function worldToLocalPosition(
  snapshot: Snapshot,
  id: string,
  world: Vector3
): { x: number; y: number; z: number } | null {
  if (!(id in snapshot)) return null
  const cache = new Map<string, Trs>()
  const t5 = originOf(snapshot, cache)
  const parentId = String(readTransform(snapshot, id).parent ?? 0)
  const parent: Trs =
    parentId === '0' || !(parentId in snapshot)
      ? { pos: Vector3.Zero(), rot: Quaternion.Identity(), scale: Vector3.One() }
      : composed(snapshot, parentId, cache, new Set())

  const parentWorldPos = Vector3.subtract(parent.pos, t5)
  const rel = Vector3.subtract(world, parentWorldPos)
  const inv = Quaternion.create(
    -parent.rot.x,
    -parent.rot.y,
    -parent.rot.z,
    parent.rot.w
  )
  const unrot = rotateVec3ByQuat(rel, inv)
  return {
    x: unrot.x / (parent.scale.x || 1),
    y: unrot.y / (parent.scale.y || 1),
    z: unrot.z / (parent.scale.z || 1)
  }
}

export function rootLocalForWorld(
  snapshot: Snapshot,
  world: Vector3
): { x: number; y: number; z: number } | null {
  const p = Vector3.add(world, originOf(snapshot, new Map()))
  return { x: p.x, y: p.y, z: p.z }
}

export function worldToLocalRotation(
  snapshot: Snapshot,
  id: string,
  world: Quaternion
): Quaternion | null {
  if (!(id in snapshot)) return null
  const cache = new Map<string, Trs>()
  const parentId = String(readTransform(snapshot, id).parent ?? 0)
  const parentRot =
    parentId === '0' || !(parentId in snapshot)
      ? Quaternion.Identity()
      : composed(snapshot, parentId, cache, new Set()).rot
  const inv = Quaternion.create(-parentRot.x, -parentRot.y, -parentRot.z, parentRot.w)
  return Quaternion.multiply(inv, world)
}

export function worldScaleOf(snapshot: Snapshot, id: string): Vector3 {
  return composed(snapshot, id, new Map(), new Set()).scale
}

export function localRelativeTo(
  snapshot: Snapshot,
  childId: string,
  parentId: string
): { position: TransformValue['position']; rotation: TransformValue['rotation']; scale: TransformValue['scale'] } {
  const cache = new Map<string, Trs>()
  const c = composed(snapshot, childId, cache, new Set())
  const p = composed(snapshot, parentId, cache, new Set())
  const invRot = Quaternion.create(-p.rot.x, -p.rot.y, -p.rot.z, p.rot.w)
  const rel = rotateVec3ByQuat(Vector3.subtract(c.pos, p.pos), invRot)
  const r = Quaternion.multiply(invRot, c.rot)
  return {
    position: {
      x: rel.x / (p.scale.x || 1),
      y: rel.y / (p.scale.y || 1),
      z: rel.z / (p.scale.z || 1)
    },
    rotation: { x: r.x, y: r.y, z: r.z, w: r.w },
    scale: {
      x: c.scale.x / (p.scale.x || 1),
      y: c.scale.y / (p.scale.y || 1),
      z: c.scale.z / (p.scale.z || 1)
    }
  }
}

function isZeroOffset(t: TransformValue): boolean {
  const p = t.position
  if (p === undefined) return true
  return Math.abs(p.x) + Math.abs(p.y) + Math.abs(p.z) < 1e-5
}

export function shouldMark(snapshot: Snapshot, id: string): boolean {
  if (Number(id) < 512) return false
  if (snapshot[id]?.[NAME_COMPONENT] === undefined) return false
  const comps = snapshot[id]
  if (comps?.UiTransform !== undefined || comps?.UiText !== undefined) return false
  const t = readTransform(snapshot, id)
  const parent = t.parent ?? 0
  if (parent !== 0 && isZeroOffset(t)) return false
  return true
}
