// Reads the node tree straight out of a GLB/glTF's JSON chunk (no Three.js): spring bones are named
// `*springbone*` and the physics editor only needs names and parent/child links.
// Kept equal to the validator manifest's springBoneToken by springBones.spec.ts.
export const SPRING_BONE_TOKEN = 'springbone'

export const isSpringBoneName = (name: string) => name.toLowerCase().includes(SPRING_BONE_TOKEN)

const GLB_MAGIC = 0x46546c67 // 'glTF'
const JSON_CHUNK_TYPE = 0x4e4f534a // 'JSON'
const GLB_HEADER_SIZE = 12
const CHUNK_HEADER_SIZE = 8

export type BoneNode = {
  name: string
  nodeId: number
  type: 'spring' | 'avatar'
  /** The node is a skin joint: a real bone, not a mesh, armature or other container node. */
  isJoint: boolean
  children: number[]
}

type GltfNode = { name?: string; children?: number[] }
type GltfSkin = { joints?: unknown }

/** The glTF JSON of a binary GLB, or of a plain .gltf; null when the bytes are neither. */
export function extractGltfJson(buffer: ArrayBuffer): Record<string, unknown> | null {
  if (buffer.byteLength >= GLB_HEADER_SIZE + CHUNK_HEADER_SIZE) {
    const view = new DataView(buffer)
    if (view.getUint32(0, true) === GLB_MAGIC) {
      const jsonLength = view.getUint32(GLB_HEADER_SIZE, true)
      const chunkType = view.getUint32(GLB_HEADER_SIZE + 4, true)
      const offset = GLB_HEADER_SIZE + CHUNK_HEADER_SIZE
      if (chunkType !== JSON_CHUNK_TYPE || offset + jsonLength > buffer.byteLength) return null
      return parseJson(new Uint8Array(buffer, offset, jsonLength))
    }
  }
  return parseJson(new Uint8Array(buffer))
}

function parseJson(bytes: Uint8Array): Record<string, unknown> | null {
  try {
    const parsed: unknown = JSON.parse(new TextDecoder().decode(bytes))
    return parsed && typeof parsed === 'object' ? (parsed as Record<string, unknown>) : null
  } catch {
    return null
  }
}

/** The nodes referenced as joints by any skin, or null when the model declares none. */
function collectJoints(skins: unknown): Set<number> | null {
  if (!Array.isArray(skins)) return null
  const joints = new Set<number>()
  for (const skin of skins as GltfSkin[]) {
    if (!Array.isArray(skin?.joints)) continue
    for (const joint of skin.joints) if (typeof joint === 'number') joints.add(joint)
  }
  return joints.size > 0 ? joints : null
}

/** Every node of the model as a bone entry, spring bones flagged by name and joints by the skins. */
export function parseSpringBones(buffer: ArrayBuffer): BoneNode[] {
  const json = extractGltfJson(buffer)
  const nodes = json?.nodes
  if (!Array.isArray(nodes)) return []
  const joints = collectJoints(json?.skins)
  return (nodes as GltfNode[]).map((node, index) => {
    const name = node.name ?? `node_${index}`
    return {
      name,
      nodeId: index,
      type: node.name && isSpringBoneName(node.name) ? 'spring' : 'avatar',
      // Without a skin there is nothing to tell bones from other nodes, so every node counts as one.
      isJoint: joints ? joints.has(index) : true,
      children: node.children ?? []
    }
  })
}
