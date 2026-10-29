export const PICK_LAYER = 128
export const GLTF = 'GltfContainer'
export const MESH_RENDERER = 'MeshRenderer'
export const MESH_COLLIDER = 'MeshCollider'

export const pickApplied = new Set<string>()
export const synthesized = new Set<string>()

export function stripPickColliders(snapshot: Record<string, Record<string, unknown>>): void {
  for (const [id, comps] of Object.entries(snapshot)) {
    const gltf = comps[GLTF] as { visibleMeshesCollisionMask?: number } | undefined
    if (gltf?.visibleMeshesCollisionMask !== undefined && (gltf.visibleMeshesCollisionMask & PICK_LAYER) !== 0) {
      gltf.visibleMeshesCollisionMask &= ~PICK_LAYER
    }
    const mc = comps[MESH_COLLIDER] as { collisionMask?: number } | undefined
    if (mc?.collisionMask !== undefined && (mc.collisionMask & PICK_LAYER) !== 0) {
      if (synthesized.has(id) && mc.collisionMask === PICK_LAYER) delete comps[MESH_COLLIDER]
      else mc.collisionMask &= ~PICK_LAYER
    }
  }
}
