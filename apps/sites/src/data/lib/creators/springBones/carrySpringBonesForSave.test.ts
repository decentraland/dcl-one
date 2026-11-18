import { describe, expect, it } from 'vitest'
import { type SpringBoneParams } from '@dcl/schemas'
import { ItemType, type Item } from '../items'
import { carrySpringBonesForSave, parseSpringBones, type BoneNode } from './index'

function glb(json: unknown): ArrayBuffer {
  const bytes = new TextEncoder().encode(JSON.stringify(json))
  const padded = Math.ceil(bytes.length / 4) * 4
  const buffer = new ArrayBuffer(12 + 8 + padded)
  const view = new DataView(buffer)
  view.setUint32(0, 0x46546c67, true)
  view.setUint32(4, 2, true)
  view.setUint32(8, buffer.byteLength, true)
  view.setUint32(12, padded, true)
  view.setUint32(16, 0x4e4f534a, true)
  new Uint8Array(buffer, 20).fill(0x20)
  new Uint8Array(buffer, 20).set(bytes)
  return buffer
}

const tail = glb({ nodes: [{ name: 'Avatar_Hips', children: [1] }, { name: 'Tail_springbone_1' }] })
const tailAndEars = glb({
  nodes: [{ name: 'Avatar_Hips', children: [1, 2] }, { name: 'Tail_springbone_1' }, { name: 'Ear_springbone_1' }]
})
const plain = glb({ nodes: [{ name: 'Avatar_Hips' }] })

const MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale'
const FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'
const tuned: SpringBoneParams = { stiffness: 3, gravityPower: 1, gravityDir: [0, -1, 0], drag: 0.1, isRoot: true }

/** Storage stand-in: saved hashes resolve to the GLBs below; a pending blob is parsed as given. */
const stored: Record<string, ArrayBuffer> = { bafyMale: tail, bafyFemale: tail }
const fetchBones = async (hash: string, blob?: Blob): Promise<BoneNode[]> =>
  parseSpringBones(blob ? await blob.arrayBuffer() : stored[hash])

function wearable(
  contents: Record<string, string>,
  springBones?: Record<string, Record<string, SpringBoneParams>>,
  type: ItemType = ItemType.WEARABLE
): Item {
  return {
    id: 'w1',
    name: 'Tail',
    description: '',
    thumbnail: 'thumbnail.png',
    owner: '0xabc',
    isPublished: false,
    isApproved: false,
    inCatalyst: false,
    type,
    data: {
      category: 'top_head',
      representations: [
        { bodyShapes: [MALE], mainFile: 'male/tail.glb', contents: ['male/tail.glb'] },
        { bodyShapes: [FEMALE], mainFile: 'female/tail.glb', contents: ['female/tail.glb'] }
      ],
      springBones: springBones && { version: 1, models: springBones }
    },
    contents,
    createdAt: 1,
    updatedAt: 1
  }
}

const saved = wearable(
  { 'male/tail.glb': 'bafyMale', 'female/tail.glb': 'bafyFemale' },
  { bafyMale: { Tail_springbone_1: tuned }, bafyFemale: { Tail_springbone_1: tuned } }
)

describe('carrySpringBonesForSave', () => {
  it('fetches the kept model, carries its params, and seeds defaults for roots new to the replaced one', async () => {
    const pending = {
      item: wearable({ 'male/tail.glb': 'bafyMale', 'female/tail.glb': 'bafyNew' }),
      blobs: { 'female/tail.glb': new Blob([tailAndEars]) }
    }
    const data = await carrySpringBonesForSave(fetchBones, saved, pending)
    expect(data?.models.bafyMale).toEqual({ Tail_springbone_1: tuned })
    expect(data?.models.bafyNew.Tail_springbone_1).toEqual(tuned)
    expect(data?.models.bafyNew.Ear_springbone_1).toMatchObject({ isRoot: true })
  })

  it('takes the params as they stand in the form over the saved ones', async () => {
    const unsaved = { ...tuned, drag: 0.9 }
    const pending = {
      item: wearable({ 'male/tail.glb': 'bafyMale', 'female/tail.glb': 'bafyNew' }),
      blobs: { 'female/tail.glb': new Blob([tail]) }
    }
    const data = await carrySpringBonesForSave(fetchBones, saved, pending, {
      bafyMale: { Tail_springbone_1: unsaved },
      bafyFemale: { Tail_springbone_1: unsaved }
    })
    expect(data?.models.bafyMale).toEqual({ Tail_springbone_1: unsaved })
    expect(data?.models.bafyNew).toEqual({ Tail_springbone_1: unsaved })
  })

  it('fills the shapes the form has no params for from the saved ones', async () => {
    const pending = {
      item: wearable({ 'male/tail.glb': 'bafyMale', 'female/tail.glb': 'bafyNew' }),
      blobs: { 'female/tail.glb': new Blob([tail]) }
    }
    const data = await carrySpringBonesForSave(fetchBones, saved, pending, {
      bafyFemale: { Tail_springbone_1: { ...tuned, drag: 0.9 } }
    })
    expect(data?.models.bafyMale).toEqual({ Tail_springbone_1: tuned })
    expect(data?.models.bafyNew.Tail_springbone_1.drag).toBe(0.9)
  })

  it('still carries by name when a replaced model cannot be fetched', async () => {
    const failing = async (_hash: string, blob?: Blob) => {
      if (!blob) throw new Error('storage down')
      return parseSpringBones(await blob.arrayBuffer())
    }
    const pending = {
      item: wearable({ 'male/tail.glb': 'bafyNewM', 'female/tail.glb': 'bafyNewF' }),
      blobs: { 'male/tail.glb': new Blob([tail]), 'female/tail.glb': new Blob([tail]) }
    }
    const data = await carrySpringBonesForSave(failing, saved, pending)
    expect(data?.models.bafyNewM).toEqual({ Tail_springbone_1: tuned })
  })

  it('drops the params when no new model has spring bones, and leaves emotes alone', async () => {
    const pending = {
      item: wearable({ 'male/tail.glb': 'bafyPlainM', 'female/tail.glb': 'bafyPlainF' }),
      blobs: { 'male/tail.glb': new Blob([plain]), 'female/tail.glb': new Blob([plain]) }
    }
    expect(await carrySpringBonesForSave(fetchBones, saved, pending)).toBeNull()
    const emote = wearable({ 'male/tail.glb': 'bafyMale' }, undefined, ItemType.EMOTE)
    expect(await carrySpringBonesForSave(fetchBones, emote, { item: emote, blobs: {} })).toBeUndefined()
  })
})
