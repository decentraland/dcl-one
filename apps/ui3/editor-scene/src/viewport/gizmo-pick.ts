import { Vector3 } from '@dcl/sdk/math'

export type Axis = 'x' | 'y' | 'z'

export type HandleKind =
  | { op: 'translate-axis'; axis: Axis }
  | { op: 'translate-plane'; normal: Axis }
  | { op: 'rotate'; axis: Axis }
  | { op: 'scale-axis'; axis: Axis }
  | { op: 'scale-uniform' }

export type Ray = { origin: Vector3; dir: Vector3 }

export type PickConfig = {
  armLength: number
  planeOffset: number
  ringRadius: number
  axisTol: number
  planeHalf: number
  ringTol: number
  centerTol: number
}

export function axisVec(a: Axis): Vector3 {
  return a === 'x' ? Vector3.Right() : a === 'y' ? Vector3.Up() : Vector3.Forward()
}

export function otherAxes(a: Axis): [Axis, Axis] {
  return a === 'x' ? ['y', 'z'] : a === 'y' ? ['x', 'z'] : ['x', 'y']
}

export function rayPlaneIntersect(
  origin: Vector3,
  dir: Vector3,
  planePoint: Vector3,
  normal: Vector3
): Vector3 | null {
  const denom = Vector3.dot(normal, dir)
  if (Math.abs(denom) < 1e-6) return null
  const t = Vector3.dot(normal, Vector3.subtract(planePoint, origin)) / denom
  if (t < 0) return null
  return Vector3.add(origin, Vector3.scale(dir, t))
}

export function rayPointDist(O: Vector3, D: Vector3, P: Vector3): { dist: number; t: number } {
  const t = Math.max(0, Vector3.dot(Vector3.subtract(P, O), D))
  return { dist: Vector3.distance(Vector3.add(O, Vector3.scale(D, t)), P), t }
}

export function raySegmentDist(
  O: Vector3,
  D: Vector3,
  A: Vector3,
  B: Vector3
): { dist: number; t: number } {
  const v = Vector3.subtract(B, A)
  const w0 = Vector3.subtract(O, A)
  const b = Vector3.dot(D, v)
  const c = Vector3.dot(v, v)
  const d = Vector3.dot(D, w0)
  const e = Vector3.dot(v, w0)
  const denom = c - b * b
  let u = denom < 1e-6 ? 0 : (e - d * b) / denom
  u = Math.max(0, Math.min(1, u))
  const t = Math.max(0, u * b - d)
  const rayPt = Vector3.add(O, Vector3.scale(D, t))
  const segPt = Vector3.add(A, Vector3.scale(v, u))
  return { dist: Vector3.distance(rayPt, segPt), t }
}

export function pickHandleKind(
  kinds: HandleKind[],
  center: Vector3,
  scale: number,
  ray: Ray,
  cfg: PickConfig
): number {
  let best = -1
  let bestT = Infinity
  let bestRank = Infinity
  const tryHit = (i: number, dist: number, t: number, tol: number, rank: number): void => {
    if (t <= 0 || dist > tol * scale) return
    if (rank < bestRank || (rank === bestRank && t < bestT)) {
      best = i
      bestT = t
      bestRank = rank
    }
  }
  for (let i = 0; i < kinds.length; i++) {
    const k = kinds[i]
    if (k.op === 'translate-axis' || k.op === 'scale-axis') {
      const end = Vector3.add(center, Vector3.scale(axisVec(k.axis), cfg.armLength * scale))
      const h = raySegmentDist(ray.origin, ray.dir, center, end)
      tryHit(i, h.dist, h.t, cfg.axisTol, 1)
    } else if (k.op === 'scale-uniform') {
      const h = rayPointDist(ray.origin, ray.dir, center)
      tryHit(i, h.dist, h.t, cfg.centerTol, 0)
    } else if (k.op === 'translate-plane') {
      const [a, b] = otherAxes(k.normal)
      const sqCenter = Vector3.add(
        center,
        Vector3.add(
          Vector3.scale(axisVec(a), cfg.planeOffset * scale),
          Vector3.scale(axisVec(b), cfg.planeOffset * scale)
        )
      )
      const hit = rayPlaneIntersect(ray.origin, ray.dir, sqCenter, axisVec(k.normal))
      if (hit !== null) {
        const rel = Vector3.subtract(hit, sqCenter)
        const half = cfg.planeHalf * scale
        const inSquare =
          Math.abs(Vector3.dot(rel, axisVec(a))) <= half &&
          Math.abs(Vector3.dot(rel, axisVec(b))) <= half
        if (inSquare) tryHit(i, 0, Vector3.dot(Vector3.subtract(hit, ray.origin), ray.dir), 1, 0)
      }
    } else {
      const hit = rayPlaneIntersect(ray.origin, ray.dir, center, axisVec(k.axis))
      if (hit !== null) {
        const radial = Math.abs(Vector3.distance(hit, center) - cfg.ringRadius * scale)
        if (radial <= cfg.ringTol * scale) {
          tryHit(i, 0, Vector3.dot(Vector3.subtract(hit, ray.origin), ray.dir), 1, 0)
        }
      }
    }
  }
  return best
}

/** Plane for dragging along an axis: contains the axis and faces the camera
 *  as squarely as possible, so the pointer ray always has a stable hit. */
export function axisDragPlaneNormal(axisDir: Vector3, camForward: Vector3): Vector3 {
  const side = Vector3.cross(axisDir, camForward)
  const normal = Vector3.cross(axisDir, side)
  const len = Vector3.length(normal)
  return len < 1e-6 ? Vector3.Up() : Vector3.scale(normal, 1 / len)
}

export function angleOnPlane(hit: Vector3, center: Vector3, normal: Vector3): number {
  const ref = Math.abs(normal.y) > 0.9 ? Vector3.Right() : Vector3.Up()
  const u = Vector3.normalize(Vector3.cross(normal, ref))
  const v = Vector3.normalize(Vector3.cross(normal, u))
  const rel = Vector3.subtract(hit, center)
  return Math.atan2(Vector3.dot(rel, v), Vector3.dot(rel, u))
}

/** Blender-style constraint: an axis handle keeps only the motion along its
 *  axis, a plane handle only the motion inside its plane. */
export function constrainTranslation(kind: HandleKind, worldDelta: Vector3): Vector3 {
  if (kind.op === 'translate-axis') {
    const dir = axisVec(kind.axis)
    return Vector3.scale(dir, Vector3.dot(worldDelta, dir))
  }
  if (kind.op === 'translate-plane') {
    const [a, b] = otherAxes(kind.normal)
    const da = axisVec(a)
    const db = axisVec(b)
    return Vector3.add(
      Vector3.scale(da, Vector3.dot(worldDelta, da)),
      Vector3.scale(db, Vector3.dot(worldDelta, db))
    )
  }
  return worldDelta
}
