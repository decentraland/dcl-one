// @vitest-environment node
import { describe, expect, it } from 'vitest'
import { hashV1, keccak256Hash } from '@dcl/hashing'
import { MappingType } from '@dcl/schemas'
import { type Collection } from './collections'
import {
  buildDeploymentForm,
  buildItemEntity,
  buildItemEntityMetadata,
  buildLinkedItemEntity,
  buildLinkedItemEntityMetadata,
  computeItemContentHash,
  computeLinkedItemContentHash,
  getEntityContent,
  getLinkedEntityContent
} from './catalystEntity'
import { ItemType, type Item } from './items'

const collection = { id: 'c1', contractAddress: '0xcontract' } as Collection
const EMPTY = 'bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku'

// A live mainnet wearable (…0x69d18b9b…:0) as the Catalyst serves it, with its on-chain content hash.
const live: Item = {
  id: 'i1',
  name: 'Paint Shirt',
  description: '',
  thumbnail: 'thumbnail.png',
  owner: '0xowner',
  isPublished: true,
  isApproved: true,
  inCatalyst: true,
  type: ItemType.WEARABLE,
  rarity: 'common',
  tokenId: '0',
  urn: 'urn:decentraland:matic:collections-v2:0xcontract:0',
  contents: { 'thumbnail.png': 'bafthumb', 'image.png': 'bafimage', 'model.glb': 'bafmodel' },
  data: {
    category: 'upper_body',
    hides: [],
    replaces: [],
    tags: ['shirt'],
    representations: [
      {
        bodyShapes: ['urn:decentraland:off-chain:base-avatars:BaseMale'],
        mainFile: 'model.glb',
        contents: ['model.glb'],
        overrideHides: [],
        overrideReplaces: []
      }
    ]
  },
  metrics: { triangles: 1, materials: 1, textures: 1, meshes: 1, bodies: 1, entities: 1 },
  createdAt: 0,
  updatedAt: 0
}

describe('getEntityContent', () => {
  it('drops the preview video, directory entries and empty files', () => {
    const item = {
      ...live,
      contents: { ...live.contents, 'video.mp4': 'bafvideo', 'folder/': 'x', 'empty.txt': EMPTY }
    }
    expect(getEntityContent(item)).toEqual(live.contents)
  })

  it('adds the generated image only when the item has none', () => {
    const withoutImage = { ...live.contents }
    delete withoutImage['image.png']
    expect(getEntityContent({ ...live, contents: withoutImage }, 'bafgen')['image.png']).toBe('bafgen')
    expect(getEntityContent(live, 'bafgen')['image.png']).toBe('bafimage')
  })
})

describe('buildItemEntityMetadata', () => {
  it('keeps the legacy key order, which the content hash depends on', () => {
    const metadata = buildItemEntityMetadata(collection, live, getEntityContent(live))
    expect(Object.keys(metadata)).toEqual([
      'id',
      'name',
      'description',
      'collectionAddress',
      'rarity',
      'i18n',
      'data',
      'image',
      'thumbnail',
      'metrics'
    ])
    expect(Object.keys(metadata.data!)).toEqual(['replaces', 'hides', 'tags', 'category', 'representations'])
  })

  it('references only files the entity deploys', () => {
    const item = {
      ...live,
      data: {
        ...live.data,
        representations: [{ ...live.data.representations[0], contents: ['model.glb', 'folder/'] }]
      }
    }
    const metadata = buildItemEntityMetadata(collection, item, getEntityContent(item))
    expect(metadata.data!.representations[0].contents).toEqual(['model.glb'])
  })

  it('refuses an unpublished item', () => {
    expect(() => buildItemEntityMetadata(collection, { ...live, tokenId: undefined }, {})).toThrow()
  })
})

describe('computeItemContentHash', () => {
  it('produces different hashes in the Qm and bafk formats', async () => {
    const content = getEntityContent(live)
    const v1 = await computeItemContentHash(collection, live, content)
    const v0 = await computeItemContentHash(collection, live, content, 'v0')
    expect(v1.startsWith('bafkrei')).toBe(true)
    expect(v0.startsWith('Qm')).toBe(true)
  })

  it('changes when the item metadata changes', async () => {
    const content = getEntityContent(live)
    const before = await computeItemContentHash(collection, live, content)
    const after = await computeItemContentHash(collection, { ...live, name: 'Renamed' }, content)
    expect(after).not.toBe(before)
  })
})

describe('buildItemEntity', () => {
  it('names the entity after the hash of its file and points it at the item urn', async () => {
    const entity = await buildItemEntity(collection, live, getEntityContent(live), 123)
    expect(entity.entityId).toBe(await hashV1(entity.entityFile))
    const parsed = JSON.parse(new TextDecoder().decode(entity.entityFile))
    expect(parsed).toMatchObject({ version: 'v3', type: 'wearable', pointers: [live.urn], timestamp: 123 })
  })
})

describe('buildDeploymentForm', () => {
  it('sends the entity, the auth chain and the files', () => {
    const entity = { entityId: 'bafentity', entityFile: new Uint8Array([1]) }
    const chain = [
      { type: 'SIGNER', payload: '0xme', signature: '' },
      { type: 'ECDSA_SIGNED_ENTITY', payload: 'bafentity', signature: '0xsig' }
    ]
    const form = buildDeploymentForm(entity, chain, new Map([['bafmodel', new Blob(['m'])]]))
    expect(form.get('entityId')).toBe('bafentity')
    expect(form.get('authChain[1][signature]')).toBe('0xsig')
    expect(form.has('bafentity')).toBe(true)
    expect(form.has('bafmodel')).toBe(true)
  })
})

const linkedCollection = {
  id: 'lc1',
  urn: 'urn:decentraland:amoy:collections-thirdparty:brand:lc1'
} as Collection

const linked: Item = {
  ...live,
  id: 'li1',
  rarity: undefined,
  tokenId: undefined,
  urn: `${linkedCollection.urn}:hat-1`,
  contents: { ...live.contents, 'video.mp4': 'bafvideo', 'folder/': 'x', 'empty.txt': EMPTY },
  mappings: { amoy: { '0xabc': [{ type: MappingType.ANY }] } }
}

const proof = { index: 1, proof: ['0xaa', '0xbb'], entityHash: 'deadbeef' }

describe('buildLinkedItemEntityMetadata', () => {
  it('keeps the legacy key order and hashes every key but the proof', () => {
    const metadata = buildLinkedItemEntityMetadata(linkedCollection, linked, proof)
    expect(Object.keys(metadata)).toEqual([
      'id',
      'name',
      'description',
      'i18n',
      'data',
      'image',
      'thumbnail',
      'metrics',
      'content',
      'mappings',
      'merkleProof'
    ])
    expect(metadata.merkleProof).toEqual({
      index: 1,
      proof: ['0xaa', '0xbb'],
      hashingKeys: [
        'id',
        'name',
        'description',
        'i18n',
        'data',
        'image',
        'thumbnail',
        'metrics',
        'content',
        'mappings'
      ],
      entityHash: 'deadbeef'
    })
    expect(Object.keys(metadata.merkleProof)).toEqual(['index', 'proof', 'hashingKeys', 'entityHash'])
  })

  it('hashes the stored contents without directories and empty files, video included', () => {
    const metadata = buildLinkedItemEntityMetadata(linkedCollection, linked, proof)
    expect(metadata.content).toEqual({ ...live.contents, 'video.mp4': 'bafvideo' })
    expect(metadata.id).toBe(linked.urn)
    expect(metadata).not.toHaveProperty('collectionAddress')
    expect(metadata).not.toHaveProperty('rarity')
  })

  it('leaves mappings out when the item has none', () => {
    const metadata = buildLinkedItemEntityMetadata(linkedCollection, { ...linked, mappings: undefined }, proof)
    expect(metadata).not.toHaveProperty('mappings')
    expect(metadata.merkleProof.hashingKeys).not.toContain('mappings')
  })

  it('refuses items outside the collection, unpublished ones and emotes', () => {
    expect(() => buildLinkedItemEntityMetadata(linkedCollection, { ...linked, urn: undefined }, proof)).toThrow()
    expect(() => buildLinkedItemEntityMetadata(linkedCollection, { ...linked, urn: live.urn }, proof)).toThrow()
    expect(() => buildLinkedItemEntityMetadata(linkedCollection, { ...linked, type: ItemType.EMOTE }, proof)).toThrow()
  })
})

describe('computeLinkedItemContentHash', () => {
  // builder-server's buildTPWearableEntityMetadata (src/Item/hashes.ts) for `linked`, after sanitizeItemContents,
  // hashed as calculateTPItemContentHash does: keccak256Hash(metadata, Object.keys(metadata)).
  const serverMetadata = {
    id: linked.urn,
    name: linked.name,
    description: linked.description,
    i18n: [{ code: 'en', text: linked.name }],
    data: {
      replaces: linked.data.replaces,
      hides: linked.data.hides,
      removesDefaultHiding: undefined,
      tags: linked.data.tags,
      category: linked.data.category,
      representations: linked.data.representations,
      blockVrmExport: undefined,
      outlineCompatible: undefined
    },
    image: 'image.png',
    thumbnail: 'thumbnail.png',
    metrics: linked.metrics,
    content: { ...live.contents, 'video.mp4': 'bafvideo' },
    mappings: linked.mappings
  }
  const SERVER_HASH = '617b25aa0fab28486a151fc299a2707e14ac00c0dc7433dbe2fb7c224c7f3bba'

  it('matches the content_hash builder-server stores for the item', () => {
    expect(keccak256Hash(serverMetadata, Object.keys(serverMetadata))).toBe(SERVER_HASH)
    expect(computeLinkedItemContentHash(linkedCollection, linked)).toBe(SERVER_HASH)
  })

  it('changes with the mappings, which are part of what curators approve', () => {
    const remapped: Item = { ...linked, mappings: { amoy: { '0xabc': [{ type: MappingType.SINGLE, id: '7' }] } } }
    expect(computeLinkedItemContentHash(linkedCollection, remapped)).not.toBe(SERVER_HASH)
  })
})

describe('buildLinkedItemEntity', () => {
  it('deploys a wearable pointed at the item urn whose files are exactly the content its metadata declares', async () => {
    const entity = await buildLinkedItemEntity(linkedCollection, linked, getLinkedEntityContent(linked), proof, 123)
    expect(entity.entityId).toBe(await hashV1(entity.entityFile))
    const parsed = JSON.parse(new TextDecoder().decode(entity.entityFile))
    expect(parsed).toMatchObject({ version: 'v3', type: 'wearable', pointers: [linked.urn], timestamp: 123 })
    expect(parsed.metadata.merkleProof.entityHash).toBe('deadbeef')
    // The Catalyst's third-party validator refuses any difference, in either direction.
    expect(
      Object.fromEntries(parsed.content.map((entry: { file: string; hash: string }) => [entry.file, entry.hash]))
    ).toEqual(parsed.metadata.content)
  })
})
