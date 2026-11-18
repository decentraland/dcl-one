import { describe, expect, it } from 'vitest'
import { countModelMetrics } from './modelMetrics'

function glb(json: object): Blob {
  const text = new TextEncoder().encode(JSON.stringify(json))
  const padded = new Uint8Array(Math.ceil(text.length / 4) * 4).fill(0x20)
  padded.set(text)
  const header = new DataView(new ArrayBuffer(20))
  header.setUint32(0, 0x46546c67, true)
  header.setUint32(4, 2, true)
  header.setUint32(8, 20 + padded.length, true)
  header.setUint32(12, padded.length, true)
  header.setUint32(16, 0x4e4f534a, true)
  return new Blob([header.buffer, padded])
}

const hat = {
  nodes: [{ mesh: 0, name: 'Hat' }, { name: 'Armature' }],
  meshes: [{ name: 'Hat', primitives: [{ indices: 1, attributes: { POSITION: 0 } }, { attributes: { POSITION: 2 } }] }],
  accessors: [{ count: 400 }, { count: 600 }, { count: 31 }],
  materials: [{}, {}],
  textures: [{}]
}

describe('countModelMetrics', () => {
  it('counts the triangles of indexed and non-indexed primitives of a GLB', async () => {
    expect(await countModelMetrics(glb(hat))).toEqual({
      triangles: 210,
      materials: 2,
      textures: 1,
      meshes: 2,
      bodies: 2,
      entities: 1
    })
  })

  it('counts strips and fans by their shared vertices, and no triangles for points or lines', async () => {
    const primitives = [5, 6, 0, 1].map(mode => ({ mode, attributes: { POSITION: 0 } }))
    const model = { ...hat, meshes: [{ name: 'Hat', primitives }], accessors: [{ count: 12 }] }
    expect(await countModelMetrics(glb(model))).toMatchObject({ triangles: 20, meshes: 4 })
  })

  it('reads a GLB whose JSON chunk is padded with NULs', async () => {
    const padded = new Uint8Array(await glb(hat).arrayBuffer())
    for (let i = padded.length - 1; padded[i] === 0x20; i--) padded[i] = 0
    expect(await countModelMetrics(new Blob([padded]))).toMatchObject({ triangles: 210 })
  })

  it('counts nothing for the malformed parts of a file', async () => {
    const model = { ...hat, accessors: [{ count: -9 }, { count: {} }, { count: 31 }], materials: 'xxxx', textures: 7 }
    expect(await countModelMetrics(glb(model))).toEqual({
      triangles: 10,
      materials: 0,
      textures: 0,
      meshes: 2,
      bodies: 2,
      entities: 1
    })
  })

  it('leaves colliders out, by node or mesh name', async () => {
    const withColliders = {
      ...hat,
      nodes: [...hat.nodes, { mesh: 1, name: 'Hat_collider' }, { mesh: 2, name: 'Box' }],
      meshes: [
        ...hat.meshes,
        { primitives: [{ attributes: { POSITION: 0 } }] },
        { name: 'MainCollider', primitives: [{ attributes: { POSITION: 0 } }] }
      ]
    }
    expect(await countModelMetrics(glb(withColliders))).toMatchObject({ triangles: 210, meshes: 2 })
  })

  it('reads a .gltf JSON file', async () => {
    const file = new Blob([JSON.stringify(hat)], { type: 'model/gltf+json' })
    expect(await countModelMetrics(file)).toMatchObject({ triangles: 210, materials: 2, textures: 1 })
  })

  it('gives up on a file that is not a model, or has no geometry', async () => {
    expect(await countModelMetrics(new Blob(['not a model']))).toBeNull()
    expect(await countModelMetrics(glb({ nodes: [{ name: 'Empty' }] }))).toBeNull()
  })
})
