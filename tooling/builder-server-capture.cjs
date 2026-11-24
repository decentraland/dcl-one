'use strict'

/**
 * Records the pinned builder-server's answers as conformance fixtures.
 * Usage: node tooling/builder-server-capture.cjs <upstream-checkout> <output-directory>
 * with CATALYST_TEST_DATABASE_URL naming a disposable PostgreSQL cluster.
 * See services/catalyst/tests/fixtures/conformance/builder/README.md.
 */

const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const http = require('node:http')
const https = require('node:https')
const crypto = require('node:crypto')
const { execFileSync } = require('node:child_process')
const { BUILDER_REVISION: REVISION, assertPinnedSource, sourceCapture, requireDatabaseUrl } = require('./social-capture-common.cjs')

const REPOSITORY = 'github.com/decentraland/builder-server'
const CLOCK = '2026-01-01T00:00:00.000Z'
const ROOT = path.resolve(__dirname, '..')
const FIXTURES = path.join(ROOT, 'services/catalyst/tests/fixtures/conformance/builder')
const GUARD = path.join(__dirname, 'builder-server-offline.cjs')
const KEPT_RESPONSE_HEADERS = ['content-type', 'cache-control', 'location']
const FAKED_GRAPH_FIELDS = new Set(['accounts', 'collections', 'items', 'rarities', 'thirdParties', 'receipts'])
const RARITIES_WITH_ORACLE = '0xa9158e22f89bb3f69c5600338895cb5fb81e5090'
const GET_RARITY_BY_NAME = 'b5262333'
const SEED_KEYS = ['wallets', 'third_parties', 'registry', 'collections', 'items', 'chain', 'collection_curations', 'item_curations', 'receipts', 'entities']
const FAKED_WHERE_KEYS = new Set(['id', 'id_in', 'id_gt', 'creator', 'managers_contains', 'minters_contains', 'managers_contains_nocase', 'collection', 'isCommitteeMember'])
const VOLATILE = '<<VOLATILE>>'
const MOVED_ITEM_FILE = 'bafkreigh2akiscaildcqabsyg3dfr6chu3fgpregiymsck7e7aqa4s52zy'

/** A client-shaped wearable `item` body for `id`, in `collectionId` (null: no collection). */
function movedItem(id, collectionId) {
  return {
    item: {
      id,
      name: 'Conformance moved',
      description: '',
      eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
      collection_id: collectionId,
      type: 'wearable',
      rarity: 'common',
      price: '0',
      thumbnail: 'thumbnail.png',
      contents: { 'model.glb': MOVED_ITEM_FILE, 'thumbnail.png': MOVED_ITEM_FILE },
      metrics: { meshes: 1, bodies: 1, materials: 1, textures: 1, triangles: 1, entities: 1 },
      data: {
        category: 'hat',
        tags: [],
        replaces: [],
        hides: [],
        representations: [{
          bodyShapes: ['urn:decentraland:off-chain:base-avatars:BaseMale'],
          mainFile: 'model.glb',
          contents: ['model.glb'],
          overrideReplaces: [],
          overrideHides: []
        }]
      }
    }
  }
}
const OWNER_INITIALIZE = '0xd14fee77000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000001400000000000000000000000000000000000000000000000000000000000000180000000000000000000000000c1aba94a79ca07d9d15a62ff5c9b382777e0c70e0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000017113b44fdd661a156cc01b5031e3acf72c32eb300000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000011436f6e666f726d616e6365207361766564000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e44434c2d434e46524d4e43535644000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004268747470733a2f2f706565722e646563656e7472616c616e642e6f72672f6c616d626461732f636f6c6c656374696f6e732f7374616e646172642f6572633732312f0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000'
const STRANGER_INITIALIZE = '0xd14fee77000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000001400000000000000000000000000000000000000000000000000000000000000180000000000000000000000000cd6884d4258edc833f31ac773e24affe6a7d31540000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000017113b44fdd661a156cc01b5031e3acf72c32eb300000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000011436f6e666f726d616e6365207361766564000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e44434c2d434e46524d4e43535644000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004268747470733a2f2f706565722e646563656e7472616c616e642e6f72672f6c616d626461732f636f6c6c656374696f6e732f7374616e646172642f6572633732312f0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000'
const ITEM_MODEL = 'bafkreigl2ikuvlnqi45jrzwowmv4uqdjd6oopfwal3usjqi5fiiwm4yqei'
const ITEM_THUMBNAIL = 'bafkreicjyysmiyonyudrmq6ejc6b7m42uma5fcnqoy6h4frcmugd7jtfma'
const ITEM_TEXTURE = 'bafkreihguhxlcxdu6hacttsy5lwuggfmehebqdb6ngpey2le77bibwl7xe'
const ITEM_MODEL_V2 = 'bafkreiefylq3hbo4poewjyrt2yheyzadw3agtgqvt2xw5jypyyygqac4k4'
const ITEM_OTHER_MODEL = 'bafkreidt4shc7ymjpyczl3qv7kuk4jf5r254fdi4lbycn5jjtiauqllvqa'
const ITEM_EMPTY = 'bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku'
const ITEM_DRAFT_MODEL = 'bafkreigtbcabv6bu27zstecfipauq63zdag7z3qmjbku3cfytp4cykvkka'
const ITEM_NEVER = 'bafkreievhgfj53hbqd5f4dgteqoeoovwkos2zkxzyjrtartamlmuyo44ay'
/** Upstream's bucket URL when no S3 endpoint overrides it (`getBucketURL()` of bucket `conformance`). */
const BUCKET_URL = 'https://conformance.s3.amazonaws.com'
const ADDRESS = {
  owner: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
  stranger: '0xcd6884d4258edc833f31ac773e24affe6a7d3154',
  committee: '0x01eba3f0e992f53debaf51ed3738dc2f23e09aee',
  manager: '0x1257bf35fafebbfc08ea47903e1f7c36ecda48c0'
}
const LINKED = 'urn:decentraland:matic:collections-thirdparty:conformance:items-linked'
const id = n => `c011ec71-0000-4000-8000-000000000${n}`

/**
 * An `item` body as the old builder web's `toRemoteItem` sends it, with every object's keys in
 * sorted order so the replay, which re-serializes the body, sends them in the same order.
 */
function itemBody(fields) {
  const item = {
    beneficiary: null,
    blockchain_item_id: null,
    collection_id: null,
    contents: { 'model.glb': ITEM_MODEL, 'thumbnail.png': ITEM_THUMBNAIL },
    data: {
      category: 'hat',
      hides: [],
      replaces: [],
      representations: [{
        bodyShapes: ['urn:decentraland:off-chain:base-avatars:BaseMale'],
        contents: ['model.glb'],
        mainFile: 'model.glb',
        overrideHides: [],
        overrideReplaces: []
      }],
      tags: ['conformance']
    },
    description: '',
    eth_address: ADDRESS.owner,
    is_approved: false,
    is_published: false,
    mappings: null,
    metrics: { bodies: 1, entities: 1, materials: 1, meshes: 1, textures: 1, triangles: 100 },
    price: null,
    rarity: 'common',
    thumbnail: 'thumbnail.png',
    total_supply: 0,
    type: 'wearable',
    urn: null,
    utility: null,
    video: null,
    ...fields
  }
  return { item: Object.fromEntries(Object.entries(item).sort(([a], [b]) => (a < b ? -1 : 1))) }
}

const SAVED = ['data.created_at', 'data.updated_at']
const UPDATED = ['data.updated_at']

const ITEM_CASES = [
  { name: '152-item-draft-owner', description: 'GET /items/{id} signed by the owner of a draft item in a draft collection', method: 'GET', path: `/v1/items/${id('3d1')}`, signer: 'owner' },
  { name: '153-item-draft-stranger', description: 'GET /items/{id} signed by a wallet with no role on a draft item', method: 'GET', path: `/v1/items/${id('3d1')}`, signer: 'stranger' },
  { name: '154-item-draft-committee', description: 'GET /items/{id} signed by a committee member on a draft item', method: 'GET', path: `/v1/items/${id('3d1')}`, signer: 'committee' },
  { name: '155-item-unminted-published-manager', description: 'GET /items/{id} signed by an on-chain manager of a collection on chain, on an item it holds that is not minted', method: 'GET', path: `/v1/items/${id('3b2')}`, signer: 'manager' },
  { name: '156-item-minted-manager', description: 'GET /items/{id} signed by an on-chain manager on a minted item whose chain price, beneficiary, rarity and supply differ from its row', method: 'GET', path: `/v1/items/${id('3b1')}`, signer: 'manager' },
  { name: '157-item-minted-minter', description: 'GET /items/{id} signed by an on-chain minter on a minted item', method: 'GET', path: `/v1/items/${id('3b1')}`, signer: 'minter' },
  { name: '158-item-minted-stranger', description: 'GET /items/{id} signed by a wallet with no role on a minted item whose chain values differ from its row', method: 'GET', path: `/v1/items/${id('3b1')}`, signer: 'stranger' },
  { name: '159-item-approved-stranger', description: 'GET /items/{id} signed by a wallet with no role on a minted item of an approved collection', method: 'GET', path: `/v1/items/${id('3b3')}`, signer: 'stranger' },
  { name: '160-item-linked-stranger', description: 'GET /items/{id} signed by a wallet with no role on a third-party item never submitted', method: 'GET', path: `/v1/items/${id('3c1')}`, signer: 'stranger' },
  { name: '161-item-linked-owner', description: 'GET /items/{id} signed by the provider manager on a third-party item', method: 'GET', path: `/v1/items/${id('3c1')}`, signer: 'owner' },
  { name: '162-item-orphan-owner', description: 'GET /items/{id} signed by the owner of an item with no collection', method: 'GET', path: `/v1/items/${id('3a1')}`, signer: 'owner' },
  { name: '163-item-orphan-stranger', description: 'GET /items/{id} signed by another wallet on an item with no collection', method: 'GET', path: `/v1/items/${id('3a1')}`, signer: 'stranger' },
  { name: '164-item-unknown', description: 'GET /items/{id} of an id no item has', method: 'GET', path: `/v1/items/${id('3ff')}`, signer: 'owner' },
  { name: '165-item-malformed-id', description: 'GET /items/{id} with an id that is not a UUID', method: 'GET', path: '/v1/items/not-a-uuid', signer: 'owner' },
  { name: '166-item-minted-admin', description: 'GET /items/{id} signed by an ADMIN_ADDRESSES wallet with no other role on a minted item', method: 'GET', path: `/v1/items/${id('3b1')}`, signer: 'admin' },
  { name: '167-collection-items-draft-owner', description: 'GET /collections/{id}/items signed by the owner of a draft collection', method: 'GET', path: `/v1/collections/${id('301')}/items`, signer: 'owner' },
  { name: '168-collection-items-draft-owner-page', description: 'GET /collections/{id}/items signed by the owner, first page of one item', method: 'GET', path: `/v1/collections/${id('301')}/items`, query: { page: '1', limit: '1' }, signer: 'owner' },
  { name: '169-collection-items-draft-owner-page-two', description: 'GET /collections/{id}/items signed by the owner, second page of one item', method: 'GET', path: `/v1/collections/${id('301')}/items`, query: { page: '2', limit: '1' }, signer: 'owner' },
  { name: '170-collection-items-published-stranger', description: 'GET /collections/{id}/items signed by a wallet with no role on a collection on chain holding a minted and an unminted item', method: 'GET', path: `/v1/collections/${id('302')}/items`, signer: 'stranger' },
  { name: '171-collection-items-published-manager', description: 'GET /collections/{id}/items signed by an on-chain manager of that collection', method: 'GET', path: `/v1/collections/${id('302')}/items`, signer: 'manager' },
  { name: '172-collection-items-published-minter', description: 'GET /collections/{id}/items signed by an on-chain minter of that collection', method: 'GET', path: `/v1/collections/${id('302')}/items`, signer: 'minter' },
  { name: '173-collection-items-by-contract', description: 'GET /collections/{contract}/items signed by the owner, naming the collection by its contract address', method: 'GET', path: '/v1/collections/0x5eed000000000000000000000000000000000302/items', signer: 'owner' },
  { name: '174-collection-items-by-contract-upper-case', description: 'GET /collections/{contract}/items signed by the owner, the contract address in upper case', method: 'GET', path: '/v1/collections/0x5EED000000000000000000000000000000000302/items', signer: 'owner' },
  { name: '175-collection-items-name', description: 'GET /collections/{id}/items signed by the owner, filtered by a part of a name', method: 'GET', path: `/v1/collections/${id('301')}/items`, query: { name: 'CAP' }, signer: 'owner' },
  { name: '176-collection-items-synced', description: 'GET /collections/{id}/items signed by the owner, filtered by synced=true', method: 'GET', path: `/v1/collections/${id('301')}/items`, query: { synced: 'true' }, signer: 'owner' },
  { name: '177-collection-items-linked-owner', description: 'GET /collections/{id}/items signed by the provider manager of a third-party collection', method: 'GET', path: `/v1/collections/${id('304')}/items`, signer: 'owner' },
  { name: '178-collection-items-linked-status', description: 'GET /collections/{id}/items of a third-party collection filtered by status=approved', method: 'GET', path: `/v1/collections/${id('304')}/items`, query: { status: 'approved' }, signer: 'owner' },
  { name: '179-collection-items-linked-missing-mapping', description: 'GET /collections/{id}/items of a third-party collection filtered by mappingStatus=missing_mapping', method: 'GET', path: `/v1/collections/${id('304')}/items`, query: { mappingStatus: 'missing_mapping' }, signer: 'owner' },
  { name: '180-collection-items-unknown-invalid-status', description: 'GET /collections/{id}/items of an id no collection has, with a status no curation has', method: 'GET', path: `/v1/collections/${id('3ff')}/items`, query: { status: 'bogus' }, signer: 'owner' },
  { name: '181-collection-items-draft-admin', description: 'GET /collections/{id}/items signed by an ADMIN_ADDRESSES wallet with no other role on a draft collection', method: 'GET', path: `/v1/collections/${id('301')}/items`, signer: 'admin' },
  { name: '182-address-items-owner-page', description: 'GET /{address}/items signed by that address, first page of 60 as the old builder web asks', method: 'GET', path: `/v1/${ADDRESS.owner}/items`, query: { page: '1', limit: '60' }, signer: 'owner' },
  { name: '183-address-items-owner-orphans', description: 'GET /{address}/items signed by that address, the orphan probe the old builder web sends', method: 'GET', path: `/v1/${ADDRESS.owner}/items`, query: { page: '1', limit: '1', collectionId: 'null' }, signer: 'owner' },
  { name: '184-address-items-owner-collection', description: 'GET /{address}/items signed by that address, filtered by one of its collections', method: 'GET', path: `/v1/${ADDRESS.owner}/items`, query: { page: '1', limit: '60', collectionId: id('301') }, signer: 'owner' },
  { name: '185-address-items-owner-unpaginated', description: 'GET /{address}/items signed by that address, without page and limit', method: 'GET', path: `/v1/${ADDRESS.owner}/items`, signer: 'owner' },
  { name: '186-address-items-other', description: 'GET /{address}/items signed by another wallet', method: 'GET', path: `/v1/${ADDRESS.owner}/items`, query: { page: '1', limit: '60' }, signer: 'stranger' },
  { name: '187-address-items-stranger-own', description: 'GET /{address}/items signed by a wallet holding one item with no collection', method: 'GET', path: `/v1/${ADDRESS.stranger}/items`, query: { page: '1', limit: '60' }, signer: 'stranger' },
  { name: '188-items-committee', description: 'GET /items signed by a committee member', method: 'GET', path: '/v1/items', signer: 'committee' },
  { name: '189-items-stranger', description: 'GET /items signed by a wallet that is not on the committee', method: 'GET', path: '/v1/items', signer: 'stranger' },
  { name: '190-item-save-draft-owner', description: 'PUT /items/{id} signed by the owner, renaming a draft item and setting its utility', method: 'PUT', path: `/v1/items/${id('3d1')}`, signer: 'owner', body: itemBody({ id: id('3d1'), collection_id: id('301'), name: 'Draft hat renamed', description: 'A conformance hat', utility: 'Conformance utility' }), volatile: UPDATED },
  { name: '191-item-create-draft-owner', description: 'PUT /items/{id} signed by the owner, creating an item in its draft collection', method: 'PUT', path: `/v1/items/${id('3e1')}`, signer: 'owner', body: itemBody({ id: id('3e1'), collection_id: id('301'), name: 'Created hat' }), volatile: SAVED },
  { name: '192-item-create-orphan-stranger', description: 'PUT /items/{id} signed by a wallet creating an item with no collection', method: 'PUT', path: `/v1/items/${id('3e2')}`, signer: 'stranger', body: itemBody({ id: id('3e2'), eth_address: ADDRESS.stranger, name: 'Stranger hat' }), volatile: SAVED },
  { name: '193-item-save-draft-stranger', description: 'PUT /items/{id} signed by a wallet with no role on the draft collection', method: 'PUT', path: `/v1/items/${id('3d1')}`, signer: 'stranger', body: itemBody({ id: id('3d1'), collection_id: id('301'), name: 'Taken hat' }) },
  { name: '194-item-save-draft-committee', description: 'PUT /items/{id} signed by a committee member on a draft item', method: 'PUT', path: `/v1/items/${id('3d1')}`, signer: 'committee', body: itemBody({ id: id('3d1'), collection_id: id('301'), name: 'Curated hat' }) },
  { name: '195-item-save-orphan-other', description: 'PUT /items/{id} signed by another wallet on an item with no collection', method: 'PUT', path: `/v1/items/${id('3a1')}`, signer: 'stranger', body: itemBody({ id: id('3a1'), eth_address: ADDRESS.stranger, name: 'Taken orphan' }) },
  { name: '196-item-save-minted-owner', description: 'PUT /items/{id} signed by the owner, renaming a minted item of a collection on chain', method: 'PUT', path: `/v1/items/${id('3b1')}`, signer: 'owner', body: itemBody({ id: id('3b1'), collection_id: id('302'), blockchain_item_id: '0', name: 'Minted hat renamed', urn: 'urn:decentraland:matic:collections-v2:0x5eed000000000000000000000000000000000302:0' }), volatile: UPDATED },
  { name: '197-item-save-minted-manager', description: 'PUT /items/{id} signed by an on-chain manager, changing a minted item\'s description', method: 'PUT', path: `/v1/items/${id('3b1')}`, signer: 'manager', body: itemBody({ id: id('3b1'), collection_id: id('302'), blockchain_item_id: '0', name: 'Minted hat renamed', description: 'Managed', urn: 'urn:decentraland:matic:collections-v2:0x5eed000000000000000000000000000000000302:0' }), volatile: UPDATED },
  { name: '198-item-save-minted-rarity', description: 'PUT /items/{id} signed by the owner, changing a minted item\'s rarity', method: 'PUT', path: `/v1/items/${id('3b1')}`, signer: 'owner', body: itemBody({ id: id('3b1'), collection_id: id('302'), blockchain_item_id: '0', name: 'Minted hat renamed', rarity: 'epic' }) },
  { name: '199-item-save-unminted-published-owner', description: 'PUT /items/{id} signed by the owner on an unminted item of a collection on chain', method: 'PUT', path: `/v1/items/${id('3b2')}`, signer: 'owner', body: itemBody({ id: id('3b2'), collection_id: id('302'), name: 'Unminted cap renamed' }), volatile: UPDATED },
  { name: '200-item-create-into-published', description: 'PUT /items/{id} signed by the owner, creating an item in a collection on chain', method: 'PUT', path: `/v1/items/${id('3e3')}`, signer: 'owner', body: itemBody({ id: id('3e3'), collection_id: id('302'), name: 'Late hat' }) },
  { name: '201-item-remove-from-published', description: 'PUT /items/{id} signed by the owner, taking an item out of a collection on chain', method: 'PUT', path: `/v1/items/${id('3b2')}`, signer: 'owner', body: itemBody({ id: id('3b2'), name: 'Unminted cap renamed' }) },
  { name: '202-item-move-into-published', description: 'PUT /items/{id} signed by the owner, moving a draft item into a collection on chain', method: 'PUT', path: `/v1/items/${id('3e1')}`, signer: 'owner', body: itemBody({ id: id('3e1'), collection_id: id('302'), name: 'Created hat' }) },
  { name: '203-item-save-unknown-collection', description: 'PUT /items/{id} signed by the owner, creating an item in a collection id no collection has', method: 'PUT', path: `/v1/items/${id('3e4')}`, signer: 'owner', body: itemBody({ id: id('3e4'), collection_id: id('3ff'), name: 'Lost hat' }) },
  { name: '204-item-save-id-mismatch', description: 'PUT /items/{id} whose body names another id', method: 'PUT', path: `/v1/items/${id('3e4')}`, signer: 'owner', body: itemBody({ id: id('3e5'), name: 'Mismatched hat' }) },
  { name: '205-item-save-representation-outside-contents', description: 'PUT /items/{id} whose representation names a file its contents lack', method: 'PUT', path: `/v1/items/${id('3e4')}`, signer: 'owner', body: itemBody({ id: id('3e4'), name: 'Hollow hat', contents: { 'thumbnail.png': ITEM_THUMBNAIL } }) },
  { name: '206-item-save-invalid-name', description: 'PUT /items/{id} whose name holds a colon', method: 'PUT', path: `/v1/items/${id('3e4')}`, signer: 'owner', body: itemBody({ id: id('3e4'), name: 'Colon: hat' }) },
  { name: '207-item-save-extra-fields', description: 'PUT /items/{id} signed by the owner with keys the schema does not name, in the item and in its data', method: 'PUT', path: `/v1/items/${id('3d1')}`, signer: 'owner', body: (() => { const body = itemBody({ id: id('3d1'), collection_id: id('301'), name: 'Draft hat renamed', description: 'A conformance hat', utility: 'Conformance utility', zzz_extra: 'dropped' }); body.item.data.zzz_extra = true; return body })(), volatile: UPDATED },
  { name: '208-item-create-too-many-tags', description: 'PUT /items/{id} signed by the owner, creating an item with 21 tags', method: 'PUT', path: `/v1/items/${id('3e4')}`, signer: 'owner', body: (() => { const body = itemBody({ id: id('3e4'), name: 'Tagged hat' }); body.item.data.tags = Array.from({ length: 21 }, (_, i) => `tag${String(i).padStart(2, '0')}`); return body })() },
  { name: '209-item-save-unsigned', description: 'PUT /items/{id} without a signature', method: 'PUT', path: `/v1/items/${id('3d1')}`, body: itemBody({ id: id('3d1'), collection_id: id('301'), name: 'Anonymous hat' }) },
  { name: '210-item-save-linked-by-urn', description: 'PUT /items/{urn} signed by the provider manager, saving a third-party item by its URN', method: 'PUT', path: `/v1/items/${LINKED}:hat-1`, signer: 'owner', body: (() => { const body = itemBody({ collection_id: id('304'), name: 'Linked hat renamed', rarity: null, urn: `${LINKED}:hat-1` }); delete body.item.id; return body })(), volatile: UPDATED },
  { name: '211-item-save-linked-by-id', description: 'PUT /items/{id} signed by the provider manager, saving a third-party item by its id with its URN', method: 'PUT', path: `/v1/items/${id('3c2')}`, signer: 'owner', body: itemBody({ id: id('3c2'), collection_id: id('304'), name: 'Linked cap renamed', rarity: null, urn: `${LINKED}:cap-2` }), volatile: UPDATED },
  { name: '212-item-save-linked-stranger', description: 'PUT /items/{id} signed by a wallet that does not manage the provider', method: 'PUT', path: `/v1/items/${id('3c2')}`, signer: 'stranger', body: itemBody({ id: id('3c2'), collection_id: id('304'), name: 'Taken cap', rarity: null, urn: `${LINKED}:cap-2` }) },
  { name: '213-item-create-linked-by-urn', description: 'PUT /items/{urn} of a URN no item has', method: 'PUT', path: `/v1/items/${LINKED}:boots-9`, signer: 'owner', body: (() => { const body = itemBody({ collection_id: id('304'), name: 'Linked boots', rarity: null, urn: `${LINKED}:boots-9` }); delete body.item.id; return body })() },
  { name: '214-item-save-linked-urn-taken', description: 'PUT /items/{id} renaming a third-party item\'s URN to one another item holds', method: 'PUT', path: `/v1/items/${id('3c2')}`, signer: 'owner', body: itemBody({ id: id('3c2'), collection_id: id('304'), name: 'Linked cap renamed', rarity: null, urn: `${LINKED}:hat-1` }) },
  { name: '215-item-create-linked', description: 'PUT /items/{id} signed by the provider manager, creating a third-party item', method: 'PUT', path: `/v1/items/${id('3e5')}`, signer: 'owner', body: itemBody({ id: id('3e5'), collection_id: id('304'), name: 'Linked scarf', rarity: null, urn: `${LINKED}:scarf-3` }), volatile: SAVED },
  { name: '216-item-create-linked-without-urn', description: 'PUT /items/{id} creating a third-party item with a null URN', method: 'PUT', path: `/v1/items/${id('3e6')}`, signer: 'owner', body: itemBody({ id: id('3e6'), collection_id: id('304'), name: 'Linked glove', rarity: null }) },
  { name: '217-item-save-linked-urn-other-case', description: 'PUT /items/{urn} naming an existing third-party item\'s URN with its item part in upper case', method: 'PUT', path: `/v1/items/${LINKED}:HAT-1`, signer: 'owner', body: (() => { const body = itemBody({ collection_id: id('304'), name: 'Linked hat shouted', rarity: null, urn: `${LINKED}:HAT-1` }); delete body.item.id; return body })() },
  { name: '218-item-save-linked-mappings', description: 'PUT /items/{id} signed by the provider manager, setting a third-party item\'s mappings', method: 'PUT', path: `/v1/items/${id('3c1')}`, signer: 'owner', body: itemBody({ id: id('3c1'), collection_id: id('304'), name: 'Linked hat renamed', rarity: null, urn: `${LINKED}:hat-1`, mappings: { mainnet: { '0x5eed00000000000000000000000000000000c0de': [{ type: 'any' }] } } }), volatile: UPDATED },
  { name: '219-item-delete-stranger', description: 'DELETE /items/{id} signed by a wallet with no role on the draft collection', method: 'DELETE', path: `/v1/items/${id('3d2')}`, signer: 'stranger' },
  { name: '220-item-delete-minted', description: 'DELETE /items/{id} signed by the owner on a minted item', method: 'DELETE', path: `/v1/items/${id('3b1')}`, signer: 'owner' },
  { name: '221-item-delete-unminted-published-manager', description: 'DELETE /items/{id} signed by an on-chain manager on an unminted item of a collection on chain', method: 'DELETE', path: `/v1/items/${id('3b2')}`, signer: 'manager' },
  { name: '222-item-delete-draft-owner', description: 'DELETE /items/{id} signed by the owner of the draft collection', method: 'DELETE', path: `/v1/items/${id('3d2')}`, signer: 'owner' },
  { name: '223-item-delete-unknown', description: 'DELETE /items/{id} of an id no item has', method: 'DELETE', path: `/v1/items/${id('3ff')}`, signer: 'owner' },
  { name: '224-item-delete-linked-owner', description: 'DELETE /items/{id} signed by the provider manager on a third-party item never submitted', method: 'DELETE', path: `/v1/items/${id('3c2')}`, signer: 'owner' },
  { name: '225-item-delete-orphan-own', description: 'DELETE /items/{id} signed by the owner of an item with no collection', method: 'DELETE', path: `/v1/items/${id('3e2')}`, signer: 'stranger' },
  { name: '226-item-files-owner', description: 'POST /items/{id}/files signed by the owner, uploading the model its item names, the field named by the file\'s hash', method: 'POST', path: `/v1/items/${id('3d1')}/files`, signer: 'owner', multipart: [{ name: ITEM_MODEL, filename: 'blob', type: 'application/octet-stream', text: 'conformance model glb' }] },
  { name: '227-item-files-two', description: 'POST /items/{id}/files signed by the owner, uploading two files its item names', method: 'POST', path: `/v1/items/${id('3e1')}/files`, signer: 'owner', multipart: [{ name: ITEM_MODEL, filename: 'blob', type: 'application/octet-stream', text: 'conformance model glb' }, { name: ITEM_THUMBNAIL, filename: 'blob', type: 'image/png', text: 'conformance thumbnail png' }] },
  { name: '228-item-files-stranger', description: 'POST /items/{id}/files signed by a wallet with no role on the item', method: 'POST', path: `/v1/items/${id('3d1')}/files`, signer: 'stranger', multipart: [{ name: ITEM_MODEL, filename: 'blob', type: 'application/octet-stream', text: 'conformance model glb' }] },
  { name: '229-item-files-committee', description: 'POST /items/{id}/files signed by a committee member', method: 'POST', path: `/v1/items/${id('3d1')}/files`, signer: 'committee', multipart: [{ name: ITEM_MODEL, filename: 'blob', type: 'application/octet-stream', text: 'conformance model glb' }] },
  { name: '230-item-files-unknown', description: 'POST /items/{id}/files of an id no item has', method: 'POST', path: `/v1/items/${id('3ff')}/files`, signer: 'owner', multipart: [{ name: ITEM_MODEL, filename: 'blob', type: 'application/octet-stream', text: 'conformance model glb' }] },
  { name: '231-item-files-unsigned', description: 'POST /items/{id}/files without a signature', method: 'POST', path: `/v1/items/${id('3d1')}/files`, multipart: [{ name: ITEM_MODEL, filename: 'blob', type: 'application/octet-stream', text: 'conformance model glb' }] },
  { name: '232-item-files-published-manager', description: 'POST /items/{id}/files signed by an on-chain manager, uploading to a minted item', method: 'POST', path: `/v1/items/${id('3b1')}/files`, signer: 'manager', multipart: [{ name: ITEM_MODEL, filename: 'blob', type: 'application/octet-stream', text: 'conformance model glb' }] },
  { name: '233-item-files-hash-mismatch', description: 'POST /items/{id}/files signed by the owner, a part whose field names a hash its bytes don\'t have', method: 'POST', path: `/v1/items/${id('3d1')}/files`, signer: 'owner', multipart: [{ name: ITEM_MODEL, filename: 'blob', type: 'application/octet-stream', text: 'conformance texture png' }] },
  { name: '234-item-files-not-named', description: 'POST /items/{id}/files signed by the owner, a file whose hash the item doesn\'t name', method: 'POST', path: `/v1/items/${id('3d1')}/files`, signer: 'owner', multipart: [{ name: ITEM_OTHER_MODEL, filename: 'blob', type: 'application/octet-stream', text: 'conformance other model glb' }] },
  { name: '235-collection-lock-owner', description: 'POST /collections/{id}/lock signed by the owner of a draft collection', method: 'POST', path: `/v1/collections/${id('305')}/lock`, signer: 'owner' },
  { name: '236-item-save-locked', description: 'PUT /items/{id} signed by the owner on an item of a locked collection', method: 'PUT', path: `/v1/items/${id('3f1')}`, signer: 'owner', body: itemBody({ id: id('3f1'), collection_id: id('305'), name: 'Locked hat renamed' }) },
  { name: '237-item-create-locked', description: 'PUT /items/{id} signed by the owner, creating an item in a locked collection', method: 'PUT', path: `/v1/items/${id('3e7')}`, signer: 'owner', body: itemBody({ id: id('3e7'), collection_id: id('305'), name: 'Locked cap' }) },
  { name: '238-item-delete-locked', description: 'DELETE /items/{id} signed by the owner on an item of a locked collection', method: 'DELETE', path: `/v1/items/${id('3f1')}`, signer: 'owner' },
  { name: '239-item-files-locked', description: 'POST /items/{id}/files signed by the owner on an item of a locked collection', method: 'POST', path: `/v1/items/${id('3f1')}/files`, signer: 'owner', multipart: [{ name: ITEM_MODEL, filename: 'blob', type: 'application/octet-stream', text: 'conformance model glb' }] },
  { name: '240-item-after-saves-owner', description: 'GET /items/{id} signed by the owner after its saves and upload', method: 'GET', path: `/v1/items/${id('3d1')}`, signer: 'owner', volatile: UPDATED },
  { name: '241-collection-items-after-saves-owner', description: 'GET /collections/{id}/items signed by the owner after its items were saved, created and deleted', method: 'GET', path: `/v1/collections/${id('301')}/items`, signer: 'owner', volatile: ['data.updated_at', 'data.created_at'] },
  { name: '242-item-linked-after-saves-owner', description: 'GET /items/{id} signed by the provider manager after its third-party item was saved with mappings', method: 'GET', path: `/v1/items/${id('3c1')}`, signer: 'owner', volatile: UPDATED },
  { name: '243-item-create-folder-and-empty-entries', description: 'PUT /items/{id} signed by the owner, creating an item whose contents name a folder entry and an empty file as the old builder web packages them', method: 'PUT', path: `/v1/items/${id('3e8')}`, signer: 'owner', body: itemBody({ id: id('3e8'), collection_id: id('301'), name: 'Packaged hat', contents: { 'male/': ITEM_EMPTY, 'model.glb': ITEM_MODEL, 'notes.txt': ITEM_EMPTY, 'thumbnail.png': ITEM_THUMBNAIL } }), volatile: SAVED },
  { name: '244-item-files-folder-and-empty-entries', description: 'POST /items/{id}/files signed by the owner, the model and the folder entry and empty file under the empty-content hash, as the old builder web sends them', method: 'POST', path: `/v1/items/${id('3e8')}/files`, signer: 'owner', multipart: [{ name: ITEM_MODEL, filename: 'blob', type: 'application/octet-stream', text: 'conformance model glb' }, { name: ITEM_EMPTY, filename: 'blob', type: 'application/octet-stream', text: '' }, { name: ITEM_EMPTY, filename: 'blob', type: 'application/octet-stream', text: '' }] },
  { name: '245-item-after-folder-and-empty-entries', description: 'GET /items/{id} signed by the owner of the item saved with a folder entry and an empty file', method: 'GET', path: `/v1/items/${id('3e8')}`, signer: 'owner', volatile: SAVED },
  { name: '246-item-create-private-draft', description: 'PUT /items/{id} signed by the owner, creating an item with no collection whose model no other item names', method: 'PUT', path: `/v1/items/${id('3e9')}`, signer: 'owner', body: itemBody({ id: id('3e9'), name: 'Private hat', contents: { 'model.glb': ITEM_DRAFT_MODEL, 'thumbnail.png': ITEM_THUMBNAIL } }), volatile: SAVED },
  { name: '247-item-files-private-draft', description: 'POST /items/{id}/files signed by the owner, uploading the model of the item with no collection', method: 'POST', path: `/v1/items/${id('3e9')}/files`, signer: 'owner', multipart: [{ name: ITEM_DRAFT_MODEL, filename: 'blob', type: 'application/octet-stream', text: 'conformance draft model glb' }] },
  { name: '248-storage-published-unsigned', description: 'GET /storage/contents/{hash} unsigned, the model a minted item names', method: 'GET', path: `/v1/storage/contents/${ITEM_MODEL}` },
  { name: '249-storage-published-head', description: 'HEAD /storage/contents/{hash} unsigned, the model a minted item names', method: 'HEAD', path: `/v1/storage/contents/${ITEM_MODEL}` },
  { name: '250-storage-draft-unsigned', description: 'GET /storage/contents/{hash} unsigned, the model only an item with no collection names', method: 'GET', path: `/v1/storage/contents/${ITEM_DRAFT_MODEL}` },
  { name: '251-storage-draft-owner', description: 'GET /storage/contents/{hash} signed by the owner, the model of its item with no collection', method: 'GET', path: `/v1/storage/contents/${ITEM_DRAFT_MODEL}`, signer: 'owner' },
  { name: '252-storage-draft-stranger', description: 'GET /storage/contents/{hash} signed by a wallet with no role on the item, the model of an item with no collection', method: 'GET', path: `/v1/storage/contents/${ITEM_DRAFT_MODEL}`, signer: 'stranger' },
  { name: '253-storage-unknown-unsigned', description: 'GET /storage/contents/{hash} unsigned, a hash nobody uploaded', method: 'GET', path: `/v1/storage/contents/${ITEM_NEVER}` },
  { name: '254-storage-exists-published-unsigned', description: 'GET /storage/contents/{hash}/exists unsigned, the model a minted item names', method: 'GET', path: `/v1/storage/contents/${ITEM_MODEL}/exists` },
  { name: '255-storage-exists-draft-unsigned', description: 'GET /storage/contents/{hash}/exists unsigned, the model only an item with no collection names', method: 'GET', path: `/v1/storage/contents/${ITEM_DRAFT_MODEL}/exists` },
  { name: '256-storage-exists-draft-owner', description: 'GET /storage/contents/{hash}/exists signed by the owner, the model of its item with no collection', method: 'GET', path: `/v1/storage/contents/${ITEM_DRAFT_MODEL}/exists`, signer: 'owner' },
  { name: '257-storage-exists-unknown-unsigned', description: 'GET /storage/contents/{hash}/exists unsigned, a hash nobody uploaded', method: 'GET', path: `/v1/storage/contents/${ITEM_NEVER}/exists` }
]

const MINTED_CONTRACT = '0x5eed000000000000000000000000000000000302'
const MINTED_CONTRACT_MIXED_CASE = '0x5EED000000000000000000000000000000000302'
const DRAFT_CONTRACT = '0x5eed000000000000000000000000000000000301'

const SMALL_READ_CASES = [
  { name: '258-info-unsigned', description: 'GET /info unsigned', method: 'GET', path: '/v1/info' },
  { name: '259-rarities-unsigned', description: 'GET /rarities unsigned, every rarity the subgraph lists with its oracle price', method: 'GET', path: '/v1/rarities' },
  { name: '260-rarity-by-name', description: 'GET /rarities/{name} unsigned, a rarity the subgraph lists', method: 'GET', path: '/v1/rarities/epic' },
  { name: '261-rarity-unknown', description: 'GET /rarities/{name} unsigned, a name the subgraph does not list', method: 'GET', path: '/v1/rarities/invalid' },
  { name: '262-item-utility-minted', description: 'GET /published-collections/{address}/items/{id}/utility unsigned, a minted item', method: 'GET', path: `/v1/published-collections/${MINTED_CONTRACT}/items/0/utility` },
  { name: '263-item-utility-mixed-case-address', description: 'GET /published-collections/{address}/items/{id}/utility unsigned, the minted item by its contract in mixed case', method: 'GET', path: `/v1/published-collections/${MINTED_CONTRACT_MIXED_CASE}/items/0/utility` },
  { name: '264-item-utility-unminted', description: 'GET /published-collections/{address}/items/{id}/utility unsigned, an id no item of the contract has', method: 'GET', path: `/v1/published-collections/${MINTED_CONTRACT}/items/1/utility` },
  { name: '265-item-contents-minted', description: 'GET /items/{address}/{id}/contents unsigned, a minted item', method: 'GET', path: `/v1/items/${MINTED_CONTRACT}/0/contents` },
  { name: '266-item-contents-mixed-case-address', description: 'GET /items/{address}/{id}/contents unsigned, the minted item by its contract in mixed case', method: 'GET', path: `/v1/items/${MINTED_CONTRACT_MIXED_CASE}/0/contents` },
  { name: '267-item-contents-unminted', description: 'GET /items/{address}/{id}/contents unsigned, an id no item of the contract has', method: 'GET', path: `/v1/items/${MINTED_CONTRACT}/1/contents` },
  { name: '268-item-contents-draft-contract', description: 'GET /items/{address}/{id}/contents unsigned, the precomputed contract of a draft collection', method: 'GET', path: `/v1/items/${DRAFT_CONTRACT}/0/contents` },
  { name: '269-item-contents-invalid-address', description: 'GET /items/{address}/{id}/contents unsigned, an address that is not one', method: 'GET', path: '/v1/items/0xa/0/contents' },
  { name: '270-item-contents-invalid-item-id', description: 'GET /items/{address}/{id}/contents unsigned, an item id that is not a number', method: 'GET', path: `/v1/items/${MINTED_CONTRACT}/aItemId/contents` },
  { name: '271-addresses-by-tag', description: 'GET /addresses unsigned, by one tag as the marketplace asks', method: 'GET', path: '/v1/addresses', query: { tag: 'conformance' } },
  { name: '272-addresses-by-tags', description: 'GET /addresses unsigned, by two tags in other case', method: 'GET', path: '/v1/addresses', query: { tag: ['Hat', 'CONFORMANCE'] } },
  { name: '273-addresses-published-by-tag', description: 'GET /addresses unsigned, published collections by tag', method: 'GET', path: '/v1/addresses', query: { tag: 'conformance', is_published: 'true' } },
  { name: '274-addresses-unfiltered', description: 'GET /addresses unsigned, with no filter', method: 'GET', path: '/v1/addresses' },
  { name: '275-addresses-published-page', description: 'GET /addresses unsigned, the first page of two published collections', method: 'GET', path: '/v1/addresses', query: { is_published: 'true', page: '1', limit: '2' } }
]

const TERMS_EMAIL = 'jane.doe@example.com'

const LIFECYCLE_CASES = [
  { name: '276-collection-publish-assigns-ids', description: 'POST /collections/{id}/publish signed by the owner once the chain holds the collection and the item it lacked an on-chain id for, assigning it', method: 'POST', path: `/v1/collections/${id('302')}/publish`, signer: 'owner', volatile: ['data.items.updated_at'] },
  { name: '277-collection-publish-again', description: 'POST /collections/{id}/publish signed by the owner again, every item holding its on-chain id', method: 'POST', path: `/v1/collections/${id('302')}/publish`, signer: 'owner', volatile: ['data.items.updated_at'] },
  { name: '278-collection-publish-stranger', description: 'POST /collections/{id}/publish signed by a wallet with no role on the collection', method: 'POST', path: `/v1/collections/${id('302')}/publish`, signer: 'stranger' },
  { name: '279-collection-publish-draft-owner', description: 'POST /collections/{id}/publish signed by the owner of a collection the chain doesn\'t hold', method: 'POST', path: `/v1/collections/${id('301')}/publish`, signer: 'owner' },
  { name: '280-collection-publish-linked', description: 'POST /collections/{id}/publish signed by the provider manager of a third-party collection, without item ids', method: 'POST', path: `/v1/collections/${id('304')}/publish`, signer: 'owner' },
  { name: '281-collection-lock-stranger', description: 'POST /collections/{id}/lock signed by a wallet with no role on the draft', method: 'POST', path: `/v1/collections/${id('301')}/lock`, signer: 'stranger' },
  { name: '282-collection-lock-unsigned', description: 'POST /collections/{id}/lock without a signature', method: 'POST', path: `/v1/collections/${id('301')}/lock` },
  { name: '283-collection-lock-published-manager', description: 'POST /collections/{id}/lock signed by an on-chain manager of a collection on chain', method: 'POST', path: `/v1/collections/${id('302')}/lock`, signer: 'manager' },
  { name: '284-collection-lock-linked-manager', description: 'POST /collections/{id}/lock signed by the provider manager of a third-party collection', method: 'POST', path: `/v1/collections/${id('304')}/lock`, signer: 'owner' },
  { name: '285-collection-lock-unknown', description: 'POST /collections/{id}/lock of an id no collection has', method: 'POST', path: `/v1/collections/${id('3ff')}/lock`, signer: 'owner' },
  { name: '286-collection-tos-owner', description: 'POST /collections/{id}/tos signed by the owner, as the old builder web sends it before deploying', method: 'POST', path: `/v1/collections/${id('301')}/tos`, signer: 'owner', body: { collection_address: '0x5eed000000000000000000000000000000000301', email: TERMS_EMAIL, event: 'publish_collection_tos' } },
  { name: '287-collection-tos-invalid-email', description: 'POST /collections/{id}/tos signed by the owner with an email that is not one', method: 'POST', path: `/v1/collections/${id('301')}/tos`, signer: 'owner', body: { email: 'not an email' } },
  { name: '288-collection-tos-stranger', description: 'POST /collections/{id}/tos signed by a wallet with no role on the draft', method: 'POST', path: `/v1/collections/${id('301')}/tos`, signer: 'stranger', body: { email: TERMS_EMAIL } },
  { name: '289-collection-tos-linked', description: 'POST /collections/{id}/tos signed by the provider manager of a third-party collection, with its items\' hashes', method: 'POST', path: `/v1/collections/${id('304')}/tos`, signer: 'owner', body: { email: TERMS_EMAIL, event: 'publish_third_party_items_tos', hashes: [ITEM_MODEL] } },
  { name: '290-collection-owner-sync-owner', description: 'POST /collections/{id}/owner/sync signed by the owner of a collection on chain', method: 'POST', path: `/v1/collections/${id('302')}/owner/sync`, signer: 'owner' },
  { name: '291-collection-owner-sync-transferred-creator', description: 'POST /collections/{id}/owner/sync signed by the chain creator of a collection transferred on chain', method: 'POST', path: `/v1/collections/${id('106')}/owner/sync`, signer: 'stranger' },
  { name: '292-collection-owner-sync-linked', description: 'POST /collections/{id}/owner/sync signed by the provider manager of a third-party collection', method: 'POST', path: `/v1/collections/${id('304')}/owner/sync`, signer: 'owner' },
  { name: '293-collection-owner-sync-stranger', description: 'POST /collections/{id}/owner/sync signed by a wallet with no role on the draft', method: 'POST', path: `/v1/collections/${id('301')}/owner/sync`, signer: 'stranger' },
  { name: '294-collection-delete-stranger', description: 'DELETE /collections/{id} signed by a wallet with no role on the draft', method: 'DELETE', path: `/v1/collections/${id('301')}`, signer: 'stranger' },
  { name: '295-collection-delete-published', description: 'DELETE /collections/{id} signed by the owner of a collection on chain', method: 'DELETE', path: `/v1/collections/${id('302')}`, signer: 'owner' },
  { name: '296-collection-delete-locked', description: 'DELETE /collections/{id} signed by the owner of a collection locked under a day ago', method: 'DELETE', path: `/v1/collections/${id('305')}`, signer: 'owner' },
  { name: '297-collection-delete-linked', description: 'DELETE /collections/{id} signed by the provider manager of a third-party collection never submitted', method: 'DELETE', path: `/v1/collections/${id('304')}`, signer: 'owner' },
  { name: '298-collection-delete-draft-owner', description: 'DELETE /collections/{id} signed by the owner of a draft, with the empty body the old builder web sends', method: 'DELETE', path: `/v1/collections/${id('301')}`, signer: 'owner', body: {} },
  { name: '299-collection-delete-unknown', description: 'DELETE /collections/{id} of an id no collection has', method: 'DELETE', path: `/v1/collections/${id('3ff')}`, signer: 'owner' },
  { name: '300-collection-after-delete', description: 'GET /collections/{id} signed by the owner of the draft it deleted', method: 'GET', path: `/v1/collections/${id('301')}`, signer: 'owner' }
]

const PROVIDER = name => `urn:decentraland:matic:collections-thirdparty:${name}`
const WALLETS = {
  owner: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
  stranger: '0xcd6884d4258edc833f31ac773e24affe6a7d3154',
  manager: '0x1257bf35fafebbfc08ea47903e1f7c36ecda48c0',
  creator: '0x05bb381a642d932037707c4a626f93fb009696b2',
  provider: '0x043b83d2010ee75cc3c0419348327eb9c6da7268'
}
const linkedSave = (suffix, signer, name, urn) => ({
  method: 'PUT',
  path: `/v1/collections/${id(suffix)}`,
  signer,
  body: {
    collection: {
      id: id(suffix),
      name,
      eth_address: WALLETS[signer],
      salt: null,
      contract_address: null,
      is_published: false,
      is_approved: false,
      minters: [],
      managers: [],
      urn
    },
    data: ''
  }
})

const THIRD_PARTY_CASES = [
  { name: '301-third-parties-registry-manager', description: 'GET /thirdParties signed by a manager the registry lists for two approved providers and an unapproved one whose virtual third party lists another wallet', method: 'GET', path: '/v1/thirdParties', signer: 'manager' },
  { name: '302-third-parties-virtual-manager', description: 'GET /thirdParties signed by a wallet that a virtual third party lists, for an unapproved provider of both, a provider with no record and an unapproved provider the registry lists another wallet for', method: 'GET', path: '/v1/thirdParties', signer: 'creator' },
  { name: '303-third-parties-stranger', description: 'GET /thirdParties signed by a wallet that manages no provider', method: 'GET', path: '/v1/thirdParties', signer: 'stranger' },
  { name: '304-third-parties-unsigned', description: 'GET /thirdParties without a signature', method: 'GET', path: '/v1/thirdParties' },
  { name: '305-third-parties-manager-query', description: 'GET /thirdParties signed by a wallet that manages no provider, naming a registry manager as manager', method: 'GET', path: '/v1/thirdParties', query: { manager: WALLETS.manager }, signer: 'stranger' },
  { name: '306-third-party-registered-stranger', description: 'GET /thirdParties/{id} signed by a wallet with no role, an approved provider the registry lists', method: 'GET', path: `/v1/thirdParties/${PROVIDER('registered')}`, signer: 'stranger' },
  { name: '307-third-party-virtual-manager', description: 'GET /thirdParties/{id} signed by the manager of a virtual third party the registry doesn\'t list', method: 'GET', path: `/v1/thirdParties/${PROVIDER('listing')}`, signer: 'creator' },
  { name: '308-third-party-virtual-stranger', description: 'GET /thirdParties/{id} signed by a wallet with no role, a virtual third party the registry doesn\'t list', method: 'GET', path: `/v1/thirdParties/${PROVIDER('listing')}`, signer: 'stranger' },
  { name: '309-third-party-unknown', description: 'GET /thirdParties/{id} signed by a registry manager, a provider neither the registry nor a virtual third party has', method: 'GET', path: `/v1/thirdParties/${PROVIDER('nobody')}`, signer: 'manager' },
  { name: '310-third-party-unapproved-stranger', description: 'GET /thirdParties/{id} signed by a wallet with no role, a provider the registry lists unapproved', method: 'GET', path: `/v1/thirdParties/${PROVIDER('pending')}`, signer: 'stranger' },
  { name: '311-third-party-slots-registry-manager', description: 'GET /thirdParties/{id}/slots signed by the second manager the registry lists for an approved provider', method: 'GET', path: `/v1/thirdParties/${PROVIDER('registered')}/slots`, signer: 'minter' },
  { name: '312-third-party-slots-stranger', description: 'GET /thirdParties/{id}/slots signed by a wallet with no role', method: 'GET', path: `/v1/thirdParties/${PROVIDER('registered')}/slots`, signer: 'stranger' },
  { name: '313-third-party-slots-virtual-manager', description: 'GET /thirdParties/{id}/slots signed by the manager of a virtual third party the registry doesn\'t list', method: 'GET', path: `/v1/thirdParties/${PROVIDER('listing')}/slots`, signer: 'creator' },
  { name: '314-third-party-slots-claimed-before-registration', description: 'GET /thirdParties/{id}/slots signed by the manager of a virtual third party of a provider the registry lists unapproved for another wallet', method: 'GET', path: `/v1/thirdParties/${PROVIDER('pending')}/slots`, signer: 'creator' },
  { name: '315-third-party-slots-unapproved-registry-manager', description: 'GET /thirdParties/{id}/slots signed by the manager the registry lists unapproved for a provider whose virtual third party lists another wallet', method: 'GET', path: `/v1/thirdParties/${PROVIDER('pending')}/slots`, signer: 'manager' },
  { name: '316-third-party-slots-unapproved-both', description: 'GET /thirdParties/{id}/slots signed by a wallet the registry lists unapproved and the virtual third party lists too', method: 'GET', path: `/v1/thirdParties/${PROVIDER('claimed')}/slots`, signer: 'creator' },
  { name: '317-collection-save-unapproved-both', description: 'PUT /collections/{id} signed by a wallet the registry lists unapproved and the virtual third party lists too, creating a collection under the provider', ...linkedSave('306', 'creator', 'Claimed hats', `${PROVIDER('claimed')}:hats`), volatile: ['data.created_at', 'data.updated_at'] },
  { name: '318-collection-unapproved-registry-only-manager', description: 'GET /collections/{id} signed by a manager the registry lists unapproved that the provider\'s virtual third party doesn\'t list', method: 'GET', path: `/v1/collections/${id('306')}`, signer: 'minter' },
  { name: '319-collection-unapproved-both', description: 'GET /collections/{id} signed by a wallet the registry lists unapproved and the virtual third party lists too', method: 'GET', path: `/v1/collections/${id('306')}`, signer: 'creator', volatile: ['data.created_at', 'data.updated_at'] },
  { name: '320-collection-save-claimed-before-registration', description: 'PUT /collections/{id} signed by the manager of a virtual third party of a provider the registry lists unapproved for another wallet', ...linkedSave('307', 'creator', 'Pending squat', `${PROVIDER('pending')}:squat`), volatile: ['data.created_at', 'data.updated_at'] },
  { name: '321-collection-save-unapproved-registry-only', description: 'PUT /collections/{id} signed by the manager the registry lists unapproved for a provider whose virtual third party lists another wallet', ...linkedSave('308', 'manager', 'Pending mine', `${PROVIDER('pending')}:mine`) },
  { name: '322-collection-save-new-provider', description: 'PUT /collections/{id} signed by a wallet that manages no provider, creating the first collection of a provider nobody has', ...linkedSave('309', 'stranger', 'Fresh first', `${PROVIDER('fresh')}:first`), volatile: ['data.created_at', 'data.updated_at'] },
  { name: '323-third-party-created-virtual', description: 'GET /thirdParties/{id} signed by the wallet whose collection save created the virtual third party', method: 'GET', path: `/v1/thirdParties/${PROVIDER('fresh')}`, signer: 'stranger' },
  { name: '324-collection-save-under-another-virtual', description: 'PUT /collections/{id} signed by a wallet a provider\'s virtual third party doesn\'t list, under that provider', ...linkedSave('30a', 'owner', 'Fresh second', `${PROVIDER('fresh')}:second`) },
  { name: '325-third-party-update-other', description: 'PATCH /thirdParties/{id} signed by a wallet the virtual third party doesn\'t list', method: 'PATCH', path: `/v1/thirdParties/${PROVIDER('fresh')}`, signer: 'owner', body: { isProgrammatic: true } },
  { name: '326-third-party-update-manager', description: 'PATCH /thirdParties/{id} signed by the virtual third party\'s manager, making it programmatic', method: 'PATCH', path: `/v1/thirdParties/${PROVIDER('fresh')}`, signer: 'stranger', body: { isProgrammatic: true } },
  { name: '327-third-party-update-invalid', description: 'PATCH /thirdParties/{id} signed by the virtual third party\'s manager with a kind that is not a boolean', method: 'PATCH', path: `/v1/thirdParties/${PROVIDER('fresh')}`, signer: 'stranger', body: { isProgrammatic: 'yes' } },
  { name: '328-third-party-update-unknown', description: 'PATCH /thirdParties/{id} of a provider no virtual third party has', method: 'PATCH', path: `/v1/thirdParties/${PROVIDER('nobody')}`, signer: 'stranger', body: { isProgrammatic: true } },
  { name: '329-third-party-updated', description: 'GET /thirdParties/{id} signed by the virtual third party\'s manager after the update', method: 'GET', path: `/v1/thirdParties/${PROVIDER('fresh')}`, signer: 'stranger' },
  { name: '330-third-party-delete-other', description: 'DELETE /thirdParties/{id} signed by a wallet the virtual third party doesn\'t list', method: 'DELETE', path: `/v1/thirdParties/${PROVIDER('fresh')}`, signer: 'owner' },
  { name: '331-third-party-delete-unregistered', description: 'DELETE /thirdParties/{id} signed by the virtual third party\'s manager before the registry lists the provider', method: 'DELETE', path: `/v1/thirdParties/${PROVIDER('fresh')}`, signer: 'stranger' },
  { name: '332-third-party-delete-unknown', description: 'DELETE /thirdParties/{id} of a provider no virtual third party has', method: 'DELETE', path: `/v1/thirdParties/${PROVIDER('nobody')}`, signer: 'stranger' },
  { name: '333-third-party-delete-registered', description: 'DELETE /thirdParties/{id} signed by the virtual third party\'s manager once the registry lists the provider', method: 'DELETE', path: `/v1/thirdParties/${PROVIDER('claimed')}`, signer: 'creator' },
  { name: '334-third-party-slots-after-delete', description: 'GET /thirdParties/{id}/slots signed by the second manager the registry lists unapproved, once the virtual third party is gone', method: 'GET', path: `/v1/thirdParties/${PROVIDER('claimed')}/slots`, signer: 'minter' },
  { name: '335-collection-delete-last-of-provider', description: 'DELETE /collections/{id} signed by the manager of a virtual third party, its provider\'s only collection', method: 'DELETE', path: `/v1/collections/${id('309')}`, signer: 'stranger' },
  { name: '336-third-party-after-last-collection', description: 'GET /thirdParties/{id} signed by the former manager once the provider\'s last collection is gone', method: 'GET', path: `/v1/thirdParties/${PROVIDER('fresh')}`, signer: 'stranger' }
]

const CURATION_CASES = [
  { name: '337-item-curated-manager', description: 'GET /items/{id} signed by the provider\'s manager, a third-party item with an older approved and a newer pending curation, whose collection has a curated item of a lower id', method: 'GET', path: `/v1/items/${id('4a2')}`, signer: 'provider' },
  { name: '338-item-curated-stranger', description: 'GET /items/{id} signed by a wallet with no role, a third-party item with curations', method: 'GET', path: `/v1/items/${id('4a2')}`, signer: 'stranger' },
  { name: '339-item-curated-unsigned', description: 'GET /items/{id} without a signature, a third-party item with curations', method: 'GET', path: `/v1/items/${id('4a2')}` },
  { name: '340-item-uncurated-in-curated-collection-stranger', description: 'GET /items/{id} signed by a wallet with no role, a never-curated third-party item of a collection with curated items', method: 'GET', path: `/v1/items/${id('4a3')}`, signer: 'stranger' },
  { name: '341-collection-items-curated-manager', description: 'GET /collections/{id}/items signed by the provider\'s manager, a third-party collection with curated and never-curated items', method: 'GET', path: `/v1/collections/${id('402')}/items`, signer: 'provider' },
  { name: '342-collection-items-curated-stranger', description: 'GET /collections/{id}/items signed by a wallet with no role, a third-party collection with curated items', method: 'GET', path: `/v1/collections/${id('402')}/items`, signer: 'stranger' },
  { name: '343-collection-curated-third-party-manager', description: 'GET /collections/{id} signed by the provider\'s manager, a third-party collection with curated items', method: 'GET', path: `/v1/collections/${id('402')}`, signer: 'provider' },
  { name: '344-collection-curated-third-party-stranger', description: 'GET /collections/{id} signed by a wallet with no role, a third-party collection with curated items', method: 'GET', path: `/v1/collections/${id('402')}`, signer: 'stranger' },
  { name: '345-collection-forum-owner', description: 'GET /collections/{id} signed by the owner of a collection on chain with a forum post and a pending curation', method: 'GET', path: `/v1/collections/${id('401')}`, signer: 'provider' },
  { name: '346-collection-forum-stranger', description: 'GET /collections/{id} signed by a wallet with no role, a collection on chain with a forum post', method: 'GET', path: `/v1/collections/${id('401')}`, signer: 'stranger' },
  { name: '347-address-collections-curated', description: 'GET /{address}/collections signed by that address, owner of a curated collection and manager of a provider with a curated collection', method: 'GET', path: `/v1/${WALLETS.provider}/collections`, signer: 'provider' },
  { name: '348-third-party-slots-curated', description: 'GET /thirdParties/{id}/slots signed by the manager the registry lists, a provider with curated items', method: 'GET', path: `/v1/thirdParties/${PROVIDER('curated')}/slots`, signer: 'provider' },
  { name: '349-collection-items-curated-status', description: 'GET /collections/{id}/items signed by the provider\'s manager, filtering a third-party collection\'s items by curation status', method: 'GET', path: `/v1/collections/${id('402')}/items`, query: { status: 'approved' }, signer: 'provider' },
  { name: '350-address-items-curated', description: 'GET /{address}/items signed by that address, the owner of curated third-party items', method: 'GET', path: `/v1/${WALLETS.provider}/items`, signer: 'provider' },
  { name: '351-item-save-curated-urn-change', description: 'PUT /items/{id} signed by the provider\'s manager, changing the URN of a curated third-party item', method: 'PUT', path: `/v1/items/${id('4a1')}`, signer: 'provider', body: itemBody({ id: id('4a1'), collection_id: id('402'), eth_address: WALLETS.provider, name: 'Curated cap', rarity: null, urn: `${PROVIDER('curated')}:curated-wearables:cap-9` }) },
  { name: '352-item-delete-curated', description: 'DELETE /items/{id} signed by the provider\'s manager, a curated third-party item', method: 'DELETE', path: `/v1/items/${id('4a1')}`, signer: 'provider' }
]

const COMMITTEE = '0x01eba3f0e992f53debaf51ed3738dc2f23e09aee'
const CREATED_ROW = ['data.id', 'data.created_at', 'data.updated_at']
const curation = (body) => ({ curation: body })
const collectionCuration = suffix => `/v1/collections/${id(suffix)}/curation`
const itemCuration = suffix => `/v1/items/${id(suffix)}/curation`

const CURATION_ROUTE_CASES = [
  { name: '353-curations-committee', description: 'GET /curations signed by a committee member: the newest curation of every collection', method: 'GET', path: '/v1/curations', signer: 'committee' },
  { name: '354-curations-stranger', description: 'GET /curations signed by a wallet that creates, manages and mints nothing', method: 'GET', path: '/v1/curations', signer: 'stranger' },
  { name: '355-curations-provider', description: 'GET /curations signed by the chain creator of a collection under review and the manager of a provider with a curated collection', method: 'GET', path: '/v1/curations', signer: 'provider' },
  { name: '356-curations-unsigned', description: 'GET /curations without a signature', method: 'GET', path: '/v1/curations' },
  { name: '357-collection-curation-committee', description: 'GET /collections/{id}/curation signed by a committee member, a collection on chain with a pending curation', method: 'GET', path: collectionCuration('401'), signer: 'committee' },
  { name: '358-collection-curation-owner', description: 'GET /collections/{id}/curation signed by the owner of a collection on chain with a pending curation', method: 'GET', path: collectionCuration('401'), signer: 'provider' },
  { name: '359-collection-curation-stranger', description: 'GET /collections/{id}/curation signed by a wallet with no role on a collection on chain', method: 'GET', path: collectionCuration('401'), signer: 'stranger' },
  { name: '360-collection-curation-draft', description: 'GET /collections/{id}/curation signed by the owner of a draft the chain doesn\'t hold', method: 'GET', path: collectionCuration('001'), signer: 'owner' },
  { name: '361-collection-curation-unknown', description: 'GET /collections/{id}/curation of an id no collection has', method: 'GET', path: collectionCuration('4ff'), signer: 'owner' },
  { name: '362-collection-curation-malformed-id', description: 'GET /collections/{id}/curation with an id that is not a UUID', method: 'GET', path: '/v1/collections/not-a-uuid/curation', signer: 'owner' },
  { name: '363-collection-curation-none-owner', description: 'GET /collections/{id}/curation signed by the owner of a collection on chain that was never curated', method: 'GET', path: collectionCuration('006'), signer: 'owner' },
  { name: '364-collection-curation-none-manager', description: 'GET /collections/{id}/curation signed by an on-chain manager of a collection on chain that was never curated', method: 'GET', path: collectionCuration('006'), signer: 'manager' },
  { name: '365-collection-curation-third-party-manager', description: 'GET /collections/{id}/curation signed by the provider\'s manager, a third-party collection with curated items and an approved curation', method: 'GET', path: collectionCuration('402'), signer: 'provider' },
  { name: '366-collection-curation-third-party-stranger', description: 'GET /collections/{id}/curation signed by a wallet with no role on a third-party collection with curated items', method: 'GET', path: collectionCuration('402'), signer: 'stranger' },
  { name: '367-collection-curation-third-party-uncurated', description: 'GET /collections/{id}/curation signed by the provider\'s manager, a third-party collection with no item curation', method: 'GET', path: collectionCuration('008'), signer: 'owner' },
  { name: '368-collection-curation-admin', description: 'GET /collections/{id}/curation signed by an ADMIN_ADDRESSES wallet with no other role, a collection on chain under review', method: 'GET', path: collectionCuration('401'), signer: 'admin' },
  { name: '369-item-curations-manager', description: 'GET /collections/{id}/itemCurations signed by the provider\'s manager: the newest curation of each curated item', method: 'GET', path: `/v1/collections/${id('402')}/itemCurations`, signer: 'provider' },
  { name: '370-item-curations-bracketed-ids', description: 'GET /collections/{id}/itemCurations signed by the provider\'s manager, naming one item as itemIds[], as the old builder web sends an array', method: 'GET', path: `/v1/collections/${id('402')}/itemCurations`, query: { 'itemIds[]': [id('4a1')] }, signer: 'provider' },
  { name: '371-item-curations-repeated-ids', description: 'GET /collections/{id}/itemCurations signed by the provider\'s manager, naming two items by a repeated itemIds', method: 'GET', path: `/v1/collections/${id('402')}/itemCurations`, query: { itemIds: [id('4a1'), id('4a2')] }, signer: 'provider' },
  { name: '372-item-curations-single-id', description: 'GET /collections/{id}/itemCurations signed by the provider\'s manager, naming one item by a single itemIds, which is no array', method: 'GET', path: `/v1/collections/${id('402')}/itemCurations`, query: { itemIds: id('4a1') }, signer: 'provider' },
  { name: '373-item-curations-committee', description: 'GET /collections/{id}/itemCurations signed by a committee member', method: 'GET', path: `/v1/collections/${id('402')}/itemCurations`, signer: 'committee' },
  { name: '374-item-curations-stranger', description: 'GET /collections/{id}/itemCurations signed by a wallet with no role', method: 'GET', path: `/v1/collections/${id('402')}/itemCurations`, signer: 'stranger' },
  { name: '375-item-curations-standard-owner', description: 'GET /collections/{id}/itemCurations signed by the owner of a standard collection on chain, which has no item curation', method: 'GET', path: `/v1/collections/${id('006')}/itemCurations`, signer: 'owner' },
  { name: '376-item-curation-pending-manager', description: 'GET /items/{id}/curation signed by the provider\'s manager, an item whose newest curation is pending', method: 'GET', path: itemCuration('4a2'), signer: 'provider' },
  { name: '377-item-curation-approved-manager', description: 'GET /items/{id}/curation signed by the provider\'s manager, an item whose curation is approved', method: 'GET', path: itemCuration('4a1'), signer: 'provider' },
  { name: '378-item-curation-none-manager', description: 'GET /items/{id}/curation signed by the provider\'s manager, a never-curated item of a curated collection', method: 'GET', path: itemCuration('4a3'), signer: 'provider' },
  { name: '379-item-curation-stranger', description: 'GET /items/{id}/curation signed by a wallet with no role', method: 'GET', path: itemCuration('4a2'), signer: 'stranger' },
  { name: '380-item-curation-committee', description: 'GET /items/{id}/curation signed by a committee member', method: 'GET', path: itemCuration('4a2'), signer: 'committee' },
  { name: '381-item-curation-orphan', description: 'GET /items/{id}/curation signed by the owner of an item with no collection', method: 'GET', path: itemCuration('3a1'), signer: 'owner' },
  { name: '382-item-curation-draft-collection', description: 'GET /items/{id}/curation signed by the owner of an item of a collection the chain doesn\'t hold', method: 'GET', path: itemCuration('3f1'), signer: 'owner' },
  { name: '383-item-curation-minted-owner', description: 'GET /items/{id}/curation signed by the owner of a minted item of a standard collection', method: 'GET', path: itemCuration('3b1'), signer: 'owner' },
  { name: '384-item-curation-minted-manager', description: 'GET /items/{id}/curation signed by an on-chain manager of a minted item\'s collection', method: 'GET', path: itemCuration('3b1'), signer: 'manager' },
  { name: '385-item-curation-minted-minter', description: 'GET /items/{id}/curation signed by an on-chain minter of a minted item\'s collection', method: 'GET', path: itemCuration('3b1'), signer: 'minter' },
  { name: '386-collection-curation-request-pending', description: 'POST /collections/{id}/curation signed by the owner while a review request is pending', method: 'POST', path: collectionCuration('401'), signer: 'provider' },
  { name: '387-collection-curation-request-stranger', description: 'POST /collections/{id}/curation signed by a wallet with no role', method: 'POST', path: collectionCuration('401'), signer: 'stranger' },
  { name: '388-collection-curation-request-draft', description: 'POST /collections/{id}/curation signed by the owner of a draft the chain doesn\'t hold', method: 'POST', path: collectionCuration('001'), signer: 'owner' },
  { name: '389-collection-curation-request-unknown', description: 'POST /collections/{id}/curation of an id no collection has', method: 'POST', path: collectionCuration('4ff'), signer: 'owner' },
  { name: '390-collection-curation-request-long-note', description: 'POST /collections/{id}/curation signed by the owner with a note longer than 2,000 characters', method: 'POST', path: collectionCuration('006'), signer: 'owner', body: curation({ note: 'n'.repeat(2001) }) },
  { name: '391-collection-curation-request-first', description: 'POST /collections/{id}/curation signed by the owner of a collection on chain, unapproved and never curated: its first review request', method: 'POST', path: collectionCuration('006'), signer: 'owner', volatile: CREATED_ROW },
  { name: '392-collection-curation-request-changes', description: 'POST /collections/{id}/curation signed by the owner of a collection approved on chain and never curated: changes submitted for review', method: 'POST', path: collectionCuration('007'), signer: 'owner', volatile: CREATED_ROW },
  { name: '393-collection-curation-request-assigned', description: 'POST /collections/{id}/curation signed by a committee member assigning itself a collection on chain that was never curated', method: 'POST', path: collectionCuration('302'), signer: 'committee', body: curation({ assignee: COMMITTEE }), volatile: CREATED_ROW },
  { name: '394-collection-curation-request-committee', description: 'POST /collections/{id}/curation signed by a committee member without an assignee on another wallet\'s collection approved on chain', method: 'POST', path: collectionCuration('303'), signer: 'committee', volatile: CREATED_ROW },
  { name: '395-curations-minter', description: 'GET /curations signed by the on-chain minter of a collection with a curation', method: 'GET', path: '/v1/curations', signer: 'minter', volatile: CREATED_ROW },
  { name: '396-collection-curation-request-third-party', description: 'POST /collections/{id}/curation signed by the provider\'s manager, a third-party collection whose curation is approved', method: 'POST', path: collectionCuration('402'), signer: 'provider', volatile: CREATED_ROW },
  { name: '397-collection-curation-update-no-curation', description: 'PATCH /collections/{id}/curation signed by a committee member with a body that has no curation', method: 'PATCH', path: collectionCuration('401'), signer: 'committee', body: {} },
  { name: '398-collection-curation-update-stranger', description: 'PATCH /collections/{id}/curation signed by a wallet with no role, approving', method: 'PATCH', path: collectionCuration('401'), signer: 'stranger', body: curation({ status: 'approved' }) },
  { name: '399-collection-curation-update-owner-status', description: 'PATCH /collections/{id}/curation signed by the owner, approving its own collection', method: 'PATCH', path: collectionCuration('401'), signer: 'provider', body: curation({ status: 'approved' }) },
  { name: '400-collection-curation-update-owner-assignee', description: 'PATCH /collections/{id}/curation signed by the owner, assigning a committee member', method: 'PATCH', path: collectionCuration('401'), signer: 'provider', body: curation({ assignee: COMMITTEE }) },
  { name: '401-collection-curation-update-assignee-not-committee', description: 'PATCH /collections/{id}/curation signed by a committee member, assigning a wallet off the committee', method: 'PATCH', path: collectionCuration('401'), signer: 'committee', body: curation({ assignee: ADDRESS.stranger }) },
  { name: '402-collection-curation-update-unknown-status', description: 'PATCH /collections/{id}/curation signed by a committee member with a status that is no curation status', method: 'PATCH', path: collectionCuration('401'), signer: 'committee', body: curation({ status: 'published' }) },
  { name: '403-collection-curation-update-extra-key', description: 'PATCH /collections/{id}/curation signed by a committee member with an unknown key beside a valid status, on a collection with no curation row', method: 'PATCH', path: collectionCuration('104'), signer: 'committee', body: curation({ extra: 1, status: 'approved' }) },
  { name: '404-collection-curation-update-none', description: 'PATCH /collections/{id}/curation signed by a committee member, approving a collection on chain that was never curated', method: 'PATCH', path: collectionCuration('104'), signer: 'committee', body: curation({ status: 'approved' }) },
  { name: '405-collection-curation-update-draft', description: 'PATCH /collections/{id}/curation signed by a committee member on a draft the chain doesn\'t hold', method: 'PATCH', path: collectionCuration('001'), signer: 'committee', body: curation({ status: 'approved' }) },
  { name: '406-collection-curation-assign-unchanged', description: 'PATCH /collections/{id}/curation signed by the assigned committee member, assigning itself again', method: 'PATCH', path: collectionCuration('401'), signer: 'committee', body: curation({ assignee: COMMITTEE }), volatile: UPDATED },
  { name: '407-collection-curation-unassign', description: 'PATCH /collections/{id}/curation signed by the assigned committee member, unassigning', method: 'PATCH', path: collectionCuration('401'), signer: 'committee', body: curation({ assignee: null }), volatile: UPDATED },
  { name: '408-collection-curation-assign', description: 'PATCH /collections/{id}/curation signed by a committee member, assigning itself', method: 'PATCH', path: collectionCuration('401'), signer: 'committee', body: curation({ assignee: COMMITTEE }), volatile: UPDATED },
  { name: '409-collection-curation-approve', description: 'PATCH /collections/{id}/curation signed by the assigned committee member, approving', method: 'PATCH', path: collectionCuration('401'), signer: 'committee', body: curation({ status: 'approved' }), volatile: UPDATED },
  { name: '410-collection-curation-approve-again', description: 'PATCH /collections/{id}/curation signed by the same committee member, approving again', method: 'PATCH', path: collectionCuration('401'), signer: 'committee', body: curation({ status: 'approved' }), volatile: UPDATED },
  { name: '411-collection-curation-reject', description: 'PATCH /collections/{id}/curation signed by a committee member, rejecting with two reasons and a message with surrounding spaces', method: 'PATCH', path: collectionCuration('401'), signer: 'committee', body: curation({ rejectionMessage: '  Needs work  ', rejectionReasons: ['clipping', 'other'], status: 'rejected' }), volatile: UPDATED },
  { name: '412-collection-curation-reject-edit', description: 'PATCH /collections/{id}/curation signed by a committee member, rejecting again with another message', method: 'PATCH', path: collectionCuration('401'), signer: 'committee', body: curation({ rejectionMessage: 'Fix the clipping', status: 'rejected' }), volatile: UPDATED },
  { name: '413-collection-curation-reject-long-message', description: 'PATCH /collections/{id}/curation signed by a committee member, rejecting with a message longer than 3,000 characters', method: 'PATCH', path: collectionCuration('401'), signer: 'committee', body: curation({ rejectionMessage: 'm'.repeat(3001), status: 'rejected' }) },
  { name: '414-collection-curation-reject-unknown-reason', description: 'PATCH /collections/{id}/curation signed by a committee member, rejecting with a reason that is not one of the twelve', method: 'PATCH', path: collectionCuration('401'), signer: 'committee', body: curation({ rejectionReasons: ['ugly'], status: 'rejected' }) },
  { name: '415-collection-curation-request-after-rejection', description: 'POST /collections/{id}/curation signed by the owner after its collection\'s rejection: a review requested again', method: 'POST', path: collectionCuration('401'), signer: 'provider', volatile: CREATED_ROW },
  { name: '416-collection-curation-approve-assigned', description: 'PATCH /collections/{id}/curation signed by the assigned committee member, approving a collection it assigned itself', method: 'PATCH', path: collectionCuration('302'), signer: 'committee', body: curation({ status: 'approved' }), volatile: CREATED_ROW },
  { name: '417-collection-curation-reopen', description: 'PATCH /collections/{id}/curation signed by a committee member, setting an approved curation back to pending', method: 'PATCH', path: collectionCuration('302'), signer: 'committee', body: curation({ status: 'pending' }), volatile: CREATED_ROW },
  { name: '418-item-curation-refresh-pending', description: 'PATCH /items/{id}/curation signed by the provider\'s manager, pending again on an item whose newest curation is pending, as the old builder web pushes changes', method: 'PATCH', path: itemCuration('4a2'), signer: 'provider', body: curation({ status: 'pending' }), volatile: UPDATED },
  { name: '419-item-curation-approve-manager', description: 'PATCH /items/{id}/curation signed by the provider\'s manager, approving', method: 'PATCH', path: itemCuration('4a2'), signer: 'provider', body: curation({ status: 'approved' }) },
  { name: '420-item-curation-update-stranger', description: 'PATCH /items/{id}/curation signed by a wallet with no role', method: 'PATCH', path: itemCuration('4a2'), signer: 'stranger', body: curation({ status: 'pending' }) },
  { name: '421-item-curation-update-assignee', description: 'PATCH /items/{id}/curation signed by a committee member, assigning itself an item curation', method: 'PATCH', path: itemCuration('4a2'), signer: 'committee', body: curation({ assignee: COMMITTEE }) },
  { name: '422-item-curation-update-none', description: 'PATCH /items/{id}/curation signed by a committee member, approving a never-curated item', method: 'PATCH', path: itemCuration('4a3'), signer: 'committee', body: curation({ status: 'approved' }) },
  { name: '423-item-curation-approve', description: 'PATCH /items/{id}/curation signed by a committee member, approving an item whose newest curation is pending', method: 'PATCH', path: itemCuration('4a2'), signer: 'committee', body: curation({ status: 'approved' }), volatile: UPDATED },
  { name: '424-item-curation-request-decided', description: 'POST /items/{id}/curation signed by the provider\'s manager, an item whose newest curation is approved: a new pending curation', method: 'POST', path: itemCuration('4a2'), signer: 'provider', volatile: CREATED_ROW },
  { name: '425-item-curation-request-pending', description: 'POST /items/{id}/curation signed by the provider\'s manager, an item whose newest curation is pending', method: 'POST', path: itemCuration('4a2'), signer: 'provider' },
  { name: '426-item-curation-request-never', description: 'POST /items/{id}/curation signed by the provider\'s manager, a never-curated item', method: 'POST', path: itemCuration('4a3'), signer: 'provider' },
  { name: '427-item-curation-request-stranger', description: 'POST /items/{id}/curation signed by a wallet with no role', method: 'POST', path: itemCuration('4a2'), signer: 'stranger' },
  { name: '428-item-curation-reopen-manager', description: 'PATCH /items/{id}/curation signed by the provider\'s manager, pending on an item whose newest curation is approved', method: 'PATCH', path: itemCuration('4a1'), signer: 'provider', body: curation({ status: 'pending' }), volatile: UPDATED },
  { name: '429-curations-committee-after', description: 'GET /curations signed by a committee member after the reviews above', method: 'GET', path: '/v1/curations', signer: 'committee', volatile: CREATED_ROW },
  { name: '430-collection-curation-after', description: 'GET /collections/{id}/curation signed by a committee member after the collection was approved, rejected and requested again', method: 'GET', path: collectionCuration('401'), signer: 'committee', volatile: CREATED_ROW }
]

const forumPost = suffix => `/v1/collections/${id(suffix)}/post`
const forumReply = suffix => `/v1/collections/${id(suffix)}/curation/post`
const ASSIGNED = `The collection has been assigned to <a target="_blank" href="https://market.example.org/accounts/${COMMITTEE}">Curator</a>`

const FORUM_CASES = [
  { name: '431-collection-post-unsigned', description: 'POST /collections/{id}/post without a signature', method: 'POST', path: forumPost('302') },
  { name: '432-collection-post-stranger', description: 'POST /collections/{id}/post signed by a wallet with no role on a collection on chain', method: 'POST', path: forumPost('302'), signer: 'stranger' },
  { name: '433-collection-post-committee', description: 'POST /collections/{id}/post signed by a committee member, who has no role on the collection', method: 'POST', path: forumPost('302'), signer: 'committee' },
  { name: '434-collection-post-unknown', description: 'POST /collections/{id}/post of an id no collection has', method: 'POST', path: forumPost('3ff'), signer: 'owner' },
  { name: '435-collection-post-draft-owner', description: 'POST /collections/{id}/post signed by the owner of a collection the chain doesn\'t hold', method: 'POST', path: forumPost('101'), signer: 'creator' },
  { name: '436-collection-post-owner', description: 'POST /collections/{id}/post signed by the owner of a collection on chain, with the title and body the old builder web sends', method: 'POST', path: forumPost('302'), signer: 'owner', body: { forumPost: { title: 'Collection \'Items on chain\' created by Owner is ready for review!', raw: '# Items on chain' } } },
  { name: '437-collection-post-again', description: 'POST /collections/{id}/post signed by the owner of a collection that has its forum post', method: 'POST', path: forumPost('302'), signer: 'owner', body: { forumPost: { title: 'Again', raw: 'Again' } } },
  { name: '438-collection-post-manager', description: 'POST /collections/{id}/post signed by an on-chain manager of a collection on chain', method: 'POST', path: forumPost('006'), signer: 'manager' },
  { name: '439-collection-post-transferred-creator', description: 'POST /collections/{id}/post signed by the chain creator of a collection whose stored owner is another wallet', method: 'POST', path: forumPost('106'), signer: 'stranger' },
  { name: '440-collection-post-curated-third-party', description: 'POST /collections/{id}/post signed by the provider\'s manager of a third-party collection with curated items', method: 'POST', path: forumPost('402'), signer: 'provider' },
  { name: '441-curation-post-unsigned', description: 'POST /collections/{id}/curation/post without a signature', method: 'POST', path: forumReply('401'), body: { forumPost: { raw: ASSIGNED } } },
  { name: '442-curation-post-owner', description: 'POST /collections/{id}/curation/post signed by the owner, who is not on the committee', method: 'POST', path: forumReply('401'), signer: 'provider', body: { forumPost: { raw: ASSIGNED } } },
  { name: '443-curation-post-unknown', description: 'POST /collections/{id}/curation/post signed by a committee member, an id no collection has', method: 'POST', path: forumReply('3ff'), signer: 'committee', body: { forumPost: { raw: ASSIGNED } } },
  { name: '444-curation-post-no-topic', description: 'POST /collections/{id}/curation/post signed by a committee member, a collection without a forum post', method: 'POST', path: forumReply('101'), signer: 'committee', body: { forumPost: { raw: ASSIGNED } } },
  { name: '445-curation-post-empty', description: 'POST /collections/{id}/curation/post signed by a committee member with an empty raw', method: 'POST', path: forumReply('401'), signer: 'committee', body: { forumPost: { raw: '' } } },
  { name: '446-curation-post-too-long', description: 'POST /collections/{id}/curation/post signed by a committee member with a raw of 10,001 characters', method: 'POST', path: forumReply('401'), signer: 'committee', body: { forumPost: { raw: 'r'.repeat(10001) } } },
  { name: '447-curation-post-assigned', description: 'POST /collections/{id}/curation/post signed by a committee member, the assignment notice the old builder web sends with the topic it reads from the link', method: 'POST', path: forumReply('401'), signer: 'committee', body: { forumPost: { topic_id: 401, raw: ASSIGNED } } },
  { name: '448-curation-post-unassigned-new-topic', description: 'POST /collections/{id}/curation/post signed by a committee member in a topic posted during the run, with another topic id in the body', method: 'POST', path: forumReply('302'), signer: 'committee', body: { forumPost: { topic_id: 401, raw: 'The collection has been unassigned.' } } },
  { name: '449-curation-post-mention', description: 'POST /collections/{id}/curation/post signed by a committee member with a raw that mentions a group', method: 'POST', path: forumReply('401'), signer: 'committee', body: { forumPost: { raw: 'The collection has been assigned to @admins' } } },
  { name: '450-collection-post-third-party-draft', description: 'POST /collections/{id}/post signed by the provider\'s manager of a third-party collection none of whose items was submitted', method: 'POST', path: forumPost('008'), signer: 'owner' },
  { name: '451-collection-post-third-party-again', description: 'POST /collections/{id}/post signed by the provider\'s manager of a third-party collection that has its forum post', method: 'POST', path: forumPost('402'), signer: 'provider' }
]

const REGISTERED = PROVIDER('registered')
const publishOf = suffix => `/v1/collections/${id(suffix)}/publish`
const approvalOf = suffix => `/v1/collections/${id(suffix)}/approvalData`
const salt32 = byte => `0x${byte.repeat(32)}`
const cheque = (signer, provider, qty, salt, sentQty) => ({ signer, provider, qty, salt, ...(sentQty === undefined ? {} : { sentQty }) })
const PUBLISHED_TP = ['data.collection.created_at', 'data.collection.updated_at', 'data.collection.reviewed_at', 'data.itemCurations.id', 'data.itemCurations.created_at', 'data.itemCurations.updated_at']

const THIRD_PARTY_PUBLISH_CASES = [
  { name: '452-collection-publish-third-party-stranger', description: 'POST /collections/{id}/publish of a third-party collection signed by a wallet that manages no provider, with a cheque it signed', method: 'POST', path: publishOf('403'), signer: 'stranger', body: { itemIds: [id('4b1')] }, cheque: cheque('stranger', REGISTERED, 1, salt32('e2')) },
  { name: '453-collection-publish-third-party-committee', description: 'POST /collections/{id}/publish of a third-party collection signed by a committee member that doesn\'t manage its provider', method: 'POST', path: publishOf('403'), signer: 'committee', body: { itemIds: [id('4b1')] }, cheque: cheque('committee', REGISTERED, 1, salt32('e3')) },
  { name: '454-collection-publish-third-party-no-cheque', description: 'POST /collections/{id}/publish of a third-party collection signed by its provider\'s manager, naming items without a cheque', method: 'POST', path: publishOf('403'), signer: 'manager', body: { itemIds: [id('4b1')] } },
  { name: '455-collection-publish-third-party-other-signer', description: 'POST /collections/{id}/publish of a third-party collection signed by its provider\'s manager with a cheque the provider\'s other manager signed', method: 'POST', path: publishOf('403'), signer: 'manager', body: { itemIds: [id('4b1'), id('4b2')] }, cheque: cheque('minter', REGISTERED, 2, salt32('e4')) },
  { name: '456-collection-publish-third-party-quantity', description: 'POST /collections/{id}/publish of a third-party collection signed by its provider\'s manager naming two items with a cheque for one', method: 'POST', path: publishOf('403'), signer: 'manager', body: { itemIds: [id('4b1'), id('4b2')] }, cheque: cheque('manager', REGISTERED, 1, salt32('e5')) },
  { name: '457-collection-publish-third-party-foreign-item', description: 'POST /collections/{id}/publish of a third-party collection signed by its provider\'s manager naming an item of another collection', method: 'POST', path: publishOf('403'), signer: 'manager', body: { itemIds: [id('4b1'), id('4a1')] }, cheque: cheque('manager', REGISTERED, 2, salt32('e6')) },
  { name: '458-collection-publish-third-party-unknown-item', description: 'POST /collections/{id}/publish of a third-party collection signed by its provider\'s manager naming no item that exists', method: 'POST', path: publishOf('403'), signer: 'manager', body: { itemIds: [id('4ff')] }, cheque: cheque('manager', REGISTERED, 1, salt32('e7')) },
  { name: '459-collection-publish-third-party-no-slots', description: 'POST /collections/{id}/publish of a third-party collection signed by its provider\'s manager naming more items than the provider has slots', method: 'POST', path: publishOf('405'), signer: 'manager', body: { itemIds: [id('4d1'), id('4d2')] }, cheque: cheque('manager', PROVIDER('scarce'), 2, salt32('e8')) },
  { name: '460-collection-publish-third-party', description: 'POST /collections/{id}/publish of a third-party collection signed by its provider\'s manager naming two items with the cheque it signed', method: 'POST', path: publishOf('403'), signer: 'manager', body: { itemIds: [id('4b1'), id('4b2')] }, cheque: cheque('manager', REGISTERED, 2, salt32('b0')), volatile: PUBLISHED_TP },
  { name: '461-collection-publish-third-party-pending', description: 'POST /collections/{id}/publish of a third-party collection signed by its provider\'s manager while its items are under review', method: 'POST', path: publishOf('403'), signer: 'manager', body: { itemIds: [id('4b2')] }, cheque: cheque('manager', REGISTERED, 1, salt32('e9')) },
  { name: '462-collection-publish-third-party-consumed', description: 'POST /collections/{id}/publish of a third-party collection signed by its provider\'s manager with the cheque the registry later consumed', method: 'POST', path: publishOf('404'), signer: 'manager', body: { itemIds: [id('4c1')] }, cheque: cheque('manager', REGISTERED, 1, salt32('c0')), volatile: PUBLISHED_TP },
  { name: '463-collection-publish-third-party-reused-salt', description: 'POST /collections/{id}/publish of a third-party collection signed by its provider\'s manager with a cheque whose salt another publication of the provider used', method: 'POST', path: publishOf('406'), signer: 'manager', body: { itemIds: [id('4e1')] }, cheque: cheque('manager', REGISTERED, 1, salt32('b0')), volatile: PUBLISHED_TP },
  { name: '464-collection-approval-data-committee', description: 'GET /collections/{id}/approvalData signed by a committee member, a third-party collection with items under review', method: 'GET', path: approvalOf('403'), signer: 'committee' },
  { name: '465-collection-approval-data-consumed', description: 'GET /collections/{id}/approvalData signed by a committee member, a third-party collection whose cheque the registry consumed', method: 'GET', path: approvalOf('404'), signer: 'committee' },
  { name: '466-collection-approval-data-manager', description: 'GET /collections/{id}/approvalData signed by the provider\'s manager, not on the committee', method: 'GET', path: approvalOf('403'), signer: 'manager' },
  { name: '467-collection-approval-data-standard', description: 'GET /collections/{id}/approvalData signed by a committee member, a standard collection', method: 'GET', path: approvalOf('302'), signer: 'committee' },
  { name: '468-collection-approval-data-unsubmitted', description: 'GET /collections/{id}/approvalData signed by a committee member, a third-party collection none of whose items was submitted', method: 'GET', path: approvalOf('008'), signer: 'committee' },
  { name: '469-collection-approval-data-unknown', description: 'GET /collections/{id}/approvalData signed by a committee member, an id no collection has', method: 'GET', path: approvalOf('3ff'), signer: 'committee' },
  { name: '470-collection-approval-data-unsigned', description: 'GET /collections/{id}/approvalData without a signature', method: 'GET', path: approvalOf('403') },
  { name: '471-collection-third-party-published-manager', description: 'GET /collections/{id} signed by the provider\'s manager after its items were submitted', method: 'GET', path: `/v1/collections/${id('403')}`, signer: 'manager', volatile: ['data.created_at', 'data.updated_at', 'data.reviewed_at'] }
,
  { name: '472-item-save-third-party-urn-deployed', description: 'PUT /items/{id} signed by the provider\'s manager, creating a third-party item under a URN the catalyst holds an entity at', method: 'PUT', path: `/v1/items/${id('4a4')}`, signer: 'provider', body: itemBody({ id: id('4a4'), collection_id: id('402'), name: 'Held helmet', rarity: null, urn: `${PROVIDER('curated')}:curated-wearables:held-4` }) },
  { name: '473-item-deployed-uncurated-stranger', description: 'GET /items/{id} signed by a wallet with no role, a third-party item the catalyst holds that was never submitted for curation', method: 'GET', path: `/v1/items/${id('4a3')}`, signer: 'stranger' },
  { name: '474-item-deployed-curated-manager', description: 'GET /items/{id} signed by the provider\'s manager, a curated third-party item the catalyst holds', method: 'GET', path: `/v1/items/${id('4a1')}`, signer: 'provider' },
  { name: '475-collection-items-deployed-stranger', description: 'GET /collections/{id}/items signed by a wallet with no role, a third-party collection with deployed items', method: 'GET', path: `/v1/collections/${id('402')}/items`, signer: 'stranger' },
  { name: '476-item-deployed-minted-owner', description: 'GET /items/{id} signed by the owner, a minted standard item the catalyst holds', method: 'GET', path: `/v1/items/${id('0b1')}`, signer: 'owner' }
]

/** A wearable validator's result for collection `collection` as builder-server's schema takes it. */
const validationResult = collection => ({
  validationId: 'c011ec71-0000-4000-8000-0000000005a1',
  collectionId: collection,
  verdict: 'passed',
  rulesVersion: '1.4.0',
  items: [{ itemId: id('3b1'), contentHash: 'bafkreigh2akiscaildcqabsyg3dfr6chu3fgpregiymsck7e7aqa4s52zy', passed: true, findings: [] }]
})
const validationResultOf = suffix => `/v1/collections/${id(suffix)}/validation-result`

const VALIDATION_CASES = [
  { name: '477-validation-result-unsigned', description: 'POST /collections/{id}/validation-result without the validator\'s signature', method: 'POST', path: validationResultOf('302'), body: validationResult(id('302')) },
  { name: '478-validation-result-wrong-secret', description: 'POST /collections/{id}/validation-result signed with another secret than the node\'s', method: 'POST', path: validationResultOf('302'), body: validationResult(id('302')), validator: { secret: 'wrong' } },
  { name: '479-validation-result-stale', description: 'POST /collections/{id}/validation-result signed five minutes and a millisecond before the node\'s clock', method: 'POST', path: validationResultOf('302'), body: validationResult(id('302')), validator: { offset: -300001 } },
  { name: '480-validation-result-another-collection', description: 'POST /collections/{id}/validation-result signed by the validator, the body naming another collection than the path', method: 'POST', path: validationResultOf('302'), body: validationResult(id('303')), validator: {} },
  { name: '481-validation-result-invalid', description: 'POST /collections/{id}/validation-result signed by the validator, a body without its items', method: 'POST', path: validationResultOf('302'), body: { ...validationResult(id('302')), items: undefined }, validator: {} },
  { name: '482-validation-result', description: 'POST /collections/{id}/validation-result signed by the validator, a result of its collection while no review flow records starts', method: 'POST', path: validationResultOf('302'), body: validationResult(id('302')), validator: {} }
]

const ITEM_VIDEO_TEXT = '\u0000\u0000\u0000\u0018ftypisom\u0000\u0000\u0002\u0000isomconf\u0000\u0000\u0000\u0008moov\u0000\u0000\u0000\u0019mdatconformance video'
const ITEM_VIDEO = 'bafkreibozaxi4ubbxpjzszp4w2t73gmtb23um46nykraewnxacb2wuzoki'
const ITEM_OTHER_VIDEO_TEXT = '\u0000\u0000\u0000\u0018ftypisom\u0000\u0000\u0002\u0000isomconf\u0000\u0000\u0000\u0008moov\u0000\u0000\u0000\u001fmdatconformance other video'
const videoPart = (text, name = 'video.mp4') => [{ name, filename: 'video.mp4', type: 'video/mp4', text }]
const videosOf = suffix => `/v1/items/${id(suffix)}/videos`

const VIDEO_CASES = [
  { name: '483-item-create-smart-wearable', description: 'PUT /items/{id} signed by the owner, creating an item with no collection that names its preview video as video.mp4', method: 'PUT', path: `/v1/items/${id('3fa')}`, signer: 'owner', body: itemBody({ id: id('3fa'), name: 'Smart hat', contents: { 'model.glb': ITEM_DRAFT_MODEL, 'thumbnail.png': ITEM_THUMBNAIL, 'video.mp4': ITEM_VIDEO }, video: ITEM_VIDEO }), volatile: SAVED },
  { name: '484-item-videos-owner', description: 'POST /items/{id}/videos signed by the owner, uploading the MP4 its item names as video.mp4 under that field', method: 'POST', path: videosOf('3fa'), signer: 'owner', multipart: videoPart(ITEM_VIDEO_TEXT) },
  { name: '485-item-videos-stranger', description: 'POST /items/{id}/videos signed by a wallet with no role on the item', method: 'POST', path: videosOf('3fa'), signer: 'stranger', multipart: videoPart(ITEM_VIDEO_TEXT) },
  { name: '486-item-videos-committee', description: 'POST /items/{id}/videos signed by a committee member', method: 'POST', path: videosOf('3fa'), signer: 'committee', multipart: videoPart(ITEM_VIDEO_TEXT) },
  { name: '487-item-videos-unsigned', description: 'POST /items/{id}/videos without a signature', method: 'POST', path: videosOf('3fa'), multipart: videoPart(ITEM_VIDEO_TEXT) },
  { name: '488-item-videos-unknown', description: 'POST /items/{id}/videos of an id no item has', method: 'POST', path: videosOf('3ff'), signer: 'owner', multipart: videoPart(ITEM_VIDEO_TEXT) },
  { name: '489-item-videos-not-mp4', description: 'POST /items/{id}/videos signed by the owner, a web page sent as video.mp4', method: 'POST', path: videosOf('3fa'), signer: 'owner', multipart: videoPart('<!DOCTYPE html><title>conformance</title>') },
  { name: '490-item-videos-field-by-hash', description: 'POST /items/{id}/videos signed by the owner, the video the item names sent under its hash as the field name', method: 'POST', path: videosOf('3fa'), signer: 'owner', multipart: videoPart(ITEM_VIDEO_TEXT, ITEM_VIDEO) },
  { name: '491-item-videos-not-named', description: 'POST /items/{id}/videos signed by the owner, an MP4 the item doesn\'t name', method: 'POST', path: videosOf('3fa'), signer: 'owner', multipart: videoPart(ITEM_OTHER_VIDEO_TEXT) }
]

const reviewOf = (suffix, route) => `/v1/collections/${id(suffix)}/${route}`
const EVENTS = ['data.results.id', 'data.results.created_at']
const EVENT = ['data.id', 'data.created_at']
const CURATION = ['data.id', 'data.created_at', 'data.updated_at']
const flowOn = { review_flow: true }

const REVIEW_CASES = [
  { name: '492-review-validations-flag-off', description: 'POST /collections/{id}/validations signed by the owner while the review flow is off', method: 'POST', path: reviewOf('302', 'validations'), signer: 'owner' },
  { name: '493-review-appeal-flag-off', description: 'POST /collections/{id}/curation/appeal signed by the owner while the review flow is off', method: 'POST', path: reviewOf('302', 'curation/appeal'), signer: 'owner', body: { note: 'Please look again' } },
  { name: '494-review-events-flag-off', description: 'GET /collections/{id}/events signed by the owner while the review flow is off', method: 'GET', path: reviewOf('302', 'events'), signer: 'owner' },
  { name: '495-review-comments-flag-off', description: 'POST /collections/{id}/comments signed by the owner while the review flow is off', method: 'POST', path: reviewOf('302', 'comments'), signer: 'owner', body: { message: 'Hello' } },
  { name: '496-review-events-owner', description: 'GET /collections/{id}/events signed by the owner with the review flow on, a collection a curator approved and reopened', method: 'GET', path: reviewOf('302', 'events'), signer: 'owner', volatile: EVENTS, ...flowOn },
  { name: '497-review-events-committee', description: 'GET /collections/{id}/events signed by a committee member with the review flow on', method: 'GET', path: reviewOf('302', 'events'), signer: 'committee', volatile: EVENTS, ...flowOn },
  { name: '498-review-events-stranger', description: 'GET /collections/{id}/events signed by a wallet with no role with the review flow on', method: 'GET', path: reviewOf('302', 'events'), signer: 'stranger', ...flowOn },
  { name: '499-review-events-unsigned', description: 'GET /collections/{id}/events without a signature with the review flow on', method: 'GET', path: reviewOf('302', 'events'), ...flowOn },
  { name: '500-review-comment-owner', description: 'POST /collections/{id}/comments signed by the owner with the review flow on, a pending review, a message with markup on two lines', method: 'POST', path: reviewOf('302', 'comments'), signer: 'owner', body: { message: '  Is the clipping fine now?\n<b>thanks</b> & bye  ' }, volatile: EVENT, ...flowOn },
  { name: '501-review-comment-committee', description: 'POST /collections/{id}/comments signed by a committee member with the review flow on, about an item of the collection', method: 'POST', path: reviewOf('302', 'comments'), signer: 'committee', body: { message: 'Checking it now', itemId: id('3b1') }, volatile: EVENT, ...flowOn },
  { name: '502-review-comment-stranger', description: 'POST /collections/{id}/comments signed by a wallet with no role with the review flow on', method: 'POST', path: reviewOf('302', 'comments'), signer: 'stranger', body: { message: 'Hello' }, ...flowOn },
  { name: '503-review-comment-blank', description: 'POST /collections/{id}/comments signed by the owner with the review flow on, a message of spaces', method: 'POST', path: reviewOf('302', 'comments'), signer: 'owner', body: { message: '   ' }, ...flowOn },
  { name: '504-review-appeal-not-rejected', description: 'POST /collections/{id}/curation/appeal signed by the owner with the review flow on, a pending review', method: 'POST', path: reviewOf('302', 'curation/appeal'), signer: 'owner', body: {}, ...flowOn },
  { name: '505-review-validations-owner', description: 'POST /collections/{id}/validations signed by the owner with the review flow on, a published collection no automated review started on', method: 'POST', path: reviewOf('302', 'validations'), signer: 'owner', volatile: EVENT, ...flowOn },
  { name: '506-review-validations-running', description: 'POST /collections/{id}/validations signed by the owner with the review flow on, while that review runs', method: 'POST', path: reviewOf('302', 'validations'), signer: 'owner', ...flowOn },
  { name: '507-review-validation-result-rejected', description: 'POST /collections/{id}/validation-result signed by the validator with the review flow on, rejecting the running review', method: 'POST', path: validationResultOf('302'), validator: {}, validation_of: id('302'), body: { validationId: 'c011ec71-0000-4000-8000-0000000005a2', collectionId: id('302'), verdict: 'rejected', rulesVersion: '1.4.0', items: [{ itemId: id('3b1'), contentHash: 'bafkreibk2rcaifdobrjwlon5bbyrn6immanlsfy5yjsa4lisz6mh6rw4dm', passed: false, findings: [{ rule: 'M-01', check: 'triangles', severity: 'error', message: 'Too many triangles', measured: 1.5e4, limit: 10000 }] }, { itemId: id('3b2'), contentHash: 'bafkreidvdvwrg5c75hsxt7ubcmfb5ppjvtkg7wun5abdakvchcujopqhi4', passed: true, findings: [] }] }, ...flowOn },
  { name: '508-review-curation-after-rejection', description: 'GET /collections/{id}/curation signed by the owner, after the validator rejected the review', method: 'GET', path: reviewOf('302', 'curation'), signer: 'owner', volatile: CURATION, ...flowOn },
  { name: '509-review-events-after-rejection', description: 'GET /collections/{id}/events signed by the owner with the review flow on, after the validator rejected the review', method: 'GET', path: reviewOf('302', 'events'), signer: 'owner', volatile: EVENTS, ...flowOn },
  { name: '510-review-appeal-owner', description: 'POST /collections/{id}/curation/appeal signed by the owner with the review flow on, a note on two lines after the validator\'s rejection', method: 'POST', path: reviewOf('302', 'curation/appeal'), signer: 'owner', body: { note: ' The clipping is intended:\nsee <the> reference. ' }, volatile: CURATION, ...flowOn },
  { name: '511-review-appeal-again', description: 'POST /collections/{id}/curation/appeal signed by the owner with the review flow on, while its appeal is open', method: 'POST', path: reviewOf('302', 'curation/appeal'), signer: 'owner', body: {}, volatile: CURATION, ...flowOn }
]

const NEWSLETTER_KEY = 'example-newsletter-key'
const NEWSLETTER_PUBLICATION = 'pub-example'
const NEWSLETTER_FAILING_EMAIL = 'fail@example.com'

const LISTED_ORIGIN = 'https://create.decentraland.example'
const CAPTURE_CORS_ORIGIN = 'regex:^https://(.{1,50}\\.)?decentraland\\.example$;https://partner.example'
const FOREIGN_ORIGIN = 'https://attacker.example'
const CORS_KEPT = ['access-control-allow-origin', 'access-control-allow-credentials', 'access-control-expose-headers', 'vary']
const PREFLIGHT_KEPT = [...CORS_KEPT, 'access-control-allow-methods', 'access-control-allow-headers', 'access-control-max-age']
const preflight = origin => ({ origin, 'access-control-request-method': 'PUT', 'access-control-request-headers': 'content-type,x-identity-auth-chain-0' })
const OWNED_DRAFT = '/v1/collections/c011ec71-0000-4000-8000-000000000001'

const EDGE_CASES = [
  { name: '512-newsletter-subscribe', description: 'POST /newsletter without a signature, an email and the source the sign-up sends', method: 'POST', path: '/v1/newsletter', body: { email: 'jane.doe@example.com', source: 'auth' } },
  { name: '513-newsletter-no-source', description: 'POST /newsletter without a signature, an email and no source', method: 'POST', path: '/v1/newsletter', body: { email: 'john.doe@example.com' } },
  { name: '514-newsletter-malformed-email', description: 'POST /newsletter without a signature, an email that is no address', method: 'POST', path: '/v1/newsletter', body: { email: 'not-an-email', source: 'auth' } },
  { name: '515-newsletter-service-fails', description: 'POST /newsletter without a signature, an email the newsletter service answers 500 to', method: 'POST', path: '/v1/newsletter', body: { email: NEWSLETTER_FAILING_EMAIL, source: 'auth' } },
  { name: '516-cors-listed-origin', description: 'GET /committee without a signature, from an origin CORS_ORIGIN lists', method: 'GET', path: '/v1/committee', send: { origin: LISTED_ORIGIN }, keep: CORS_KEPT },
  { name: '517-cors-foreign-origin', description: 'GET /committee without a signature, from an origin CORS_ORIGIN does not list', method: 'GET', path: '/v1/committee', send: { origin: FOREIGN_ORIGIN }, keep: CORS_KEPT },
  { name: '518-cors-preflight-listed', description: 'OPTIONS /collections/{id}, the preflight of a signed PUT from an origin CORS_ORIGIN lists', method: 'OPTIONS', path: OWNED_DRAFT, send: preflight(LISTED_ORIGIN), keep: PREFLIGHT_KEPT },
  { name: '519-cors-preflight-foreign', description: 'OPTIONS /collections/{id}, the preflight of a signed PUT from an origin CORS_ORIGIN does not list', method: 'OPTIONS', path: OWNED_DRAFT, send: preflight(FOREIGN_ORIGIN), keep: PREFLIGHT_KEPT },
  { name: '520-cors-info-any-origin', description: 'GET /info from an origin CORS_ORIGIN does not list, a route that answers every origin', method: 'GET', path: '/v1/info', send: { origin: FOREIGN_ORIGIN }, keep: CORS_KEPT },
  { name: '521-review-writes-per-minute', description: 'POST /collections/{id}/validations signed by a wallet with no role, after 30 such requests within the minute, with the review flow off', method: 'POST', path: reviewOf('302', 'validations'), signer: 'stranger', repeat: 30, keep: ['retry-after', 'ratelimit', 'ratelimit-policy'] }
]

const OWNER_COLLECTIONS = '/v1/0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e/collections'

const LISTED_TIMES = ['data.updated_at', 'data.last_activity_at']

const LISTING_CASES = [
  { name: '522-address-collections-under-review', description: 'GET /{address}/collections signed by that address, filtered to collections under review after the run requested reviews, one approved on chain with changes under review included', method: 'GET', path: OWNER_COLLECTIONS, query: { status: 'under_review' }, signer: 'owner', volatile: LISTED_TIMES },
  { name: '523-collection-curation-reject-approved', description: 'PATCH /collections/{id}/curation signed by a committee member, rejecting the changes requested on a collection the chain shows approved', method: 'PATCH', path: collectionCuration('007'), signer: 'committee', body: curation({ rejectionMessage: 'Conformance rejection', rejectionReasons: ['other'], status: 'rejected' }), volatile: CREATED_ROW },
  { name: '524-address-collections-rejected', description: 'GET /{address}/collections signed by that address, filtered to rejected collections at the contract of one approved on chain whose newest curation is a rejection', method: 'GET', path: OWNER_COLLECTIONS, query: { status: 'rejected', contract_address: '0x5eed000000000000000000000000000000000007' }, signer: 'owner', volatile: LISTED_TIMES },
  { name: '525-collections-committee-requested-at-curated', description: 'GET /collections signed by a committee member, collections whose reviews were requested in another order than they were created, sorted by REQUESTED_AT_DESC', method: 'GET', path: '/v1/collections', query: { q: 'requested', sort: 'REQUESTED_AT_DESC' }, signer: 'committee' },
  { name: '526-collections-committee-most-relevant-curated', description: 'GET /collections signed by a committee member, collections to review, under review, approved and rejected, created in another order, sorted by MOST_RELEVANT', method: 'GET', path: '/v1/collections', query: { q: 'requested', sort: 'MOST_RELEVANT' }, signer: 'committee' }
]

const report = process.stderr.write.bind(process.stderr)
let activeDrop

const CASES = [
  {
    name: '01-collections-public-search',
    description: 'GET /collections unsigned, searching published collections',
    method: 'GET',
    path: '/v1/collections',
    query: { q: 'conformance', is_published: 'true' }
  },
  {
    name: '02-collection-owner',
    description: 'GET /collections/{id} signed by the owner of a draft',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'owner'
  },
  {
    name: '03-collection-unsigned',
    description: 'GET /collections/{id} without a signature',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001'
  },
  {
    name: '04-collection-stranger',
    description: 'GET /collections/{id} signed by a wallet that holds no role on the draft',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'stranger'
  },
  {
    name: '05-collection-create',
    description: 'PUT /collections/{id} signed by the owner, creating a draft',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000002',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000002',
        name: 'Conformance new draft',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: '0x00'
    },
    volatile: ['data.created_at', 'data.updated_at']
  },
  {
    name: '06-committee-unsigned',
    description: 'GET /committee without a signature',
    method: 'GET',
    path: '/v1/committee'
  },
  {
    name: '07-collection-malformed-chain',
    description: 'GET /collections/{id} with an auth chain header that is not JSON',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    headers: { 'x-identity-auth-chain-0': '{not json' }
  },
  {
    name: '08-collection-expired-signature',
    description: 'GET /collections/{id} signed by the owner 40 minutes before the request',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'owner',
    sign: { offset: -40 * 60 * 1000 }
  },
  {
    name: '09-collection-future-signature',
    description: 'GET /collections/{id} signed by the owner 40 minutes after the request',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'owner',
    sign: { offset: 40 * 60 * 1000 }
  },
  {
    name: '10-collection-signature-for-another-path',
    description: 'GET /collections/{id} signed by the owner over another collection path',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'owner',
    sign: { path: '/v1/collections/c011ec71-0000-4000-8000-000000000009' }
  },
  {
    name: '11-collection-metadata-mismatch',
    description: 'GET /collections/{id} signed by the owner over other metadata than the request sends',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'owner',
    sign: { sentMetadata: '{"origin":"https://builder.example.org"}' }
  },
  {
    name: '12-collection-scene-signer',
    description: 'GET /collections/{id} signed by the owner with a scene signer in the metadata',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'owner',
    metadata: '{"signer":"decentraland-kernel-scene"}'
  },
  {
    name: '13-collection-non-canonical-signer',
    description: 'GET /collections/{id} signed by the owner with a signer key that is not canonical',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'owner',
    metadata: '{"Signer":"dcl:explorer"}'
  },
  {
    name: '14-collection-untimestamped-read',
    description: 'GET /collections/{id} signed by the owner over method and path alone, without timestamp or metadata headers',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'owner',
    sign: { untimestamped: true }
  },
  {
    name: '15-collection-untimestamped-write',
    description: 'PUT /collections/{id} signed by the owner over method and path alone, without timestamp or metadata headers',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000003',
    signer: 'owner',
    sign: { untimestamped: true },
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000003',
        name: 'Conformance untimestamped draft',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: '0x00'
    },
    volatile: ['data.created_at', 'data.updated_at']
  },
  {
    name: '16-collection-invalid-body',
    description: 'PUT /collections/{id} signed by the owner with a collection whose name is not a string',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000004',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000004',
        name: 5,
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: '0x00'
    }
  },
  {
    name: '17-collection-signature-for-another-method',
    description: 'GET /collections/{id} signed by the owner as a PUT',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'owner',
    sign: { method: 'PUT' }
  },
  {
    name: '18-collection-untimestamped-with-headers',
    description: 'GET /collections/{id} signed by the owner over method and path alone, sent with timestamp and metadata headers',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'owner',
    sign: { untimestamped: true, withHeaders: true }
  },
  {
    name: '19-collection-unsigned-invalid-body',
    description: 'PUT /collections/{id} without a signature and with a collection whose name is not a string',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000005',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000005',
        name: 5,
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: '0x00'
    }
  },
  {
    name: '20-collection-published-unapproved-owner',
    description: 'GET /collections/{id} signed by the owner of a collection on chain and not approved',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000006',
    signer: 'owner'
  },
  {
    name: '21-collection-published-approved-owner',
    description: 'GET /collections/{id} signed by the owner of a collection on chain and approved',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000007',
    signer: 'owner'
  },
  {
    name: '22-collection-committee',
    description: 'GET /collections/{id} signed by a committee member who holds no other role on the draft',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'committee'
  },
  {
    name: '23-collection-published-stranger',
    description: 'GET /collections/{id} signed by a wallet that holds no role on a collection on chain',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000006',
    signer: 'stranger'
  },
  {
    name: '24-collection-published-manager',
    description: 'GET /collections/{id} signed by an on-chain manager of the collection',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000006',
    signer: 'manager'
  },
  {
    name: '25-collection-save-stranger',
    description: 'PUT /collections/{id} signed by a wallet that holds no role on the draft',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'stranger',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000001',
        name: 'Conformance draft',
        eth_address: '0xcd6884d4258edc833f31ac773e24affe6a7d3154',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: '0x00'
    }
  },
  {
    name: '26-collection-save-committee',
    description: 'PUT /collections/{id} signed by a committee member who holds no other role on the draft',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'committee',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000001',
        name: 'Conformance draft',
        eth_address: '0x01eba3f0e992f53debaf51ed3738dc2f23e09aee',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: '0x00'
    }
  },
  {
    name: '27-collection-save-published-manager',
    description: 'PUT /collections/{id} signed by an on-chain manager of a collection on chain',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000006',
    signer: 'manager',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000006',
        name: 'Chain unapproved',
        eth_address: '0x1257bf35fafebbfc08ea47903e1f7c36ecda48c0',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: '0x00'
    }
  },
  {
    name: '28-collection-items-unsigned',
    description: 'GET /collections/{id}/items without a signature',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001/items'
  },
  {
    name: '29-collection-items-stranger',
    description: 'GET /collections/{id}/items signed by a wallet that holds no role on the draft',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001/items',
    signer: 'stranger'
  },
  {
    name: '30-collection-items-committee',
    description: 'GET /collections/{id}/items signed by a committee member who holds no other role on the draft',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001/items',
    signer: 'committee'
  },
  {
    name: '31-collection-items-published-stranger',
    description: 'GET /collections/{id}/items signed by a wallet that holds no role on a collection on chain',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000006/items',
    signer: 'stranger'
  },
  {
    name: '32-collection-items-published-manager',
    description: 'GET /collections/{id}/items signed by an on-chain manager of the collection',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000006/items',
    signer: 'manager'
  },
  {
    name: '33-item-orphan-into-third-party',
    description: 'PUT /items/{id} signed by the owner moving its item with no collection into a third-party collection it manages',
    method: 'PUT',
    path: '/v1/items/c011ec71-0000-4000-8000-0000000000a1',
    signer: 'owner',
    body: movedItem('c011ec71-0000-4000-8000-0000000000a1', 'c011ec71-0000-4000-8000-000000000008')
  },
  {
    name: '34-item-out-of-third-party',
    description: 'PUT /items/{id} signed by the owner taking its item out of a third-party collection it manages',
    method: 'PUT',
    path: '/v1/items/c011ec71-0000-4000-8000-0000000000a2',
    signer: 'owner',
    body: movedItem('c011ec71-0000-4000-8000-0000000000a2', null)
  },
  {
    name: '35-item-between-third-party',
    description: 'PUT /items/{id} signed by the owner moving its item from one third-party collection it manages to another',
    method: 'PUT',
    path: '/v1/items/c011ec71-0000-4000-8000-0000000000a3',
    signer: 'owner',
    body: movedItem('c011ec71-0000-4000-8000-0000000000a3', 'c011ec71-0000-4000-8000-000000000008')
  },
  {
    name: '36-item-published-stranger',
    description: 'GET /items/{id} signed by a wallet that holds no role on a minted item of a collection on chain',
    method: 'GET',
    path: '/v1/items/c011ec71-0000-4000-8000-0000000000b1',
    signer: 'stranger'
  },
  {
    name: '37-item-published-owner',
    description: 'GET /items/{id} signed by the owner of a minted item of a collection on chain',
    method: 'GET',
    path: '/v1/items/c011ec71-0000-4000-8000-0000000000b1',
    signer: 'owner'
  },
  {
    name: '38-item-published-committee',
    description: 'GET /items/{id} signed by a committee member who holds no other role on a minted item of a collection on chain',
    method: 'GET',
    path: '/v1/items/c011ec71-0000-4000-8000-0000000000b1',
    signer: 'committee'
  },
  {
    name: '39-collections-committee-search',
    description: 'GET /collections signed by a committee member, searching listing collections',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing'
    },
    signer: 'committee'
  },
  {
    name: '40-collections-committee-page',
    description: 'GET /collections signed by a committee member, paginated, first page of two',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      page: '1',
      limit: '2'
    },
    signer: 'committee'
  },
  {
    name: '41-collections-committee-name-desc',
    description: 'GET /collections signed by a committee member, sorted by name descending',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      sort: 'NAME_DESC'
    },
    signer: 'committee'
  },
  {
    name: '42-collections-committee-most-relevant',
    description: 'GET /collections signed by a committee member, sorted by relevance',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      sort: 'MOST_RELEVANT'
    },
    signer: 'committee'
  },
  {
    name: '43-collections-committee-last-activity',
    description: 'GET /collections signed by a committee member, sorted by last activity, newest first',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      sort: 'LAST_ACTIVITY_DESC'
    },
    signer: 'committee'
  },
  {
    name: '44-collections-committee-requested-at',
    description: 'GET /collections signed by a committee member, sorted by request date, oldest first',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      sort: 'REQUESTED_AT_ASC'
    },
    signer: 'committee'
  },
  {
    name: '45-collections-committee-to-review',
    description: 'GET /collections signed by a committee member, filtered to collections to review',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      status: 'to_review'
    },
    signer: 'committee'
  },
  {
    name: '46-collections-committee-statuses',
    description: 'GET /collections signed by a committee member, filtered to disabled or approved collections',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      status: [
        'disabled',
        'approved'
      ]
    },
    signer: 'committee'
  },
  {
    name: '47-collections-committee-pending',
    description: 'GET /collections signed by a committee member, filtered to pending collections',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      status: 'pending'
    },
    signer: 'committee'
  },
  {
    name: '48-collections-committee-updates',
    description: 'GET /collections signed by a committee member, filtered to reviews of updates',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      review_type: 'updates'
    },
    signer: 'committee'
  },
  {
    name: '49-collections-committee-unassigned',
    description: 'GET /collections signed by a committee member, filtered to unassigned standard collections',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      assignee: 'unassigned',
      type: 'standard'
    },
    signer: 'committee'
  },
  {
    name: '50-collections-committee-published',
    description: 'GET /collections signed by a committee member, filtered to published collections',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      is_published: 'true'
    },
    signer: 'committee'
  },
  {
    name: '51-collections-committee-unpublished',
    description: 'GET /collections signed by a committee member, filtered to unpublished collections',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      is_published: 'false'
    },
    signer: 'committee'
  },
  {
    name: '52-collections-committee-tag',
    description: 'GET /collections signed by a committee member, filtered by an item tag in another case',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      tag: 'CONFORMANCE'
    },
    signer: 'committee'
  },
  {
    name: '53-collections-committee-empty-page',
    description: 'GET /collections signed by a committee member, paginated with no match',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'nothing-matches',
      page: '1',
      limit: '5'
    },
    signer: 'committee'
  },
  {
    name: '54-collections-committee-invalid-status',
    description: 'GET /collections signed by a committee member, with an unknown status',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      status: 'nope'
    },
    signer: 'committee'
  },
  {
    name: '55-collections-committee-two-review-types',
    description: 'GET /collections signed by a committee member, with two review types',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      review_type: [
        'new',
        'updates'
      ]
    },
    signer: 'committee'
  },
  {
    name: '56-collections-committee-invalid-assignee',
    description: 'GET /collections signed by a committee member, with an assignee that is not an address',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      assignee: 'not-an-address'
    },
    signer: 'committee'
  },
  {
    name: '57-collections-public-search-listing',
    description: 'GET /collections unsigned, searching published listing collections',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      is_published: 'true'
    }
  },
  {
    name: '58-collections-public-search-approved',
    description: 'GET /collections unsigned, as the marketplace searches approved collections',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      is_published: 'true',
      status: 'approved',
      limit: '5'
    }
  },
  {
    name: '59-collections-public-search-committee',
    description: 'GET /collections signed by a committee member, searching published collections',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      is_published: 'true'
    },
    signer: 'committee'
  },
  {
    name: '60-collections-stranger',
    description: 'GET /collections signed by a wallet that is not on the committee, without a search',
    method: 'GET',
    path: '/v1/collections',
    signer: 'stranger'
  },
  {
    name: '61-collections-unsigned',
    description: 'GET /collections without a signature or a search',
    method: 'GET',
    path: '/v1/collections'
  },
  {
    name: '62-address-collections-creator',
    description: 'GET /{address}/collections signed by that address, every collection it may list',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    signer: 'creator'
  },
  {
    name: '63-address-collections-page',
    description: 'GET /{address}/collections signed by that address, paginated, first page of two',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      page: '1',
      limit: '2'
    },
    signer: 'creator'
  },
  {
    name: '64-address-collections-draft',
    description: 'GET /{address}/collections signed by that address, filtered to drafts',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      status: 'draft'
    },
    signer: 'creator'
  },
  {
    name: '65-address-collections-published',
    description: 'GET /{address}/collections signed by that address, filtered to published collections',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      status: 'published'
    },
    signer: 'creator'
  },
  {
    name: '66-address-collections-disabled',
    description: 'GET /{address}/collections signed by that address, filtered to disabled collections',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      status: 'disabled'
    },
    signer: 'creator'
  },
  {
    name: '67-address-collections-unpublished',
    description: 'GET /{address}/collections signed by that address, filtered to unpublished collections',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      is_published: 'false'
    },
    signer: 'creator'
  },
  {
    name: '68-address-collections-third-party',
    description: 'GET /{address}/collections signed by that address, filtered to third-party collections',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      type: 'third_party'
    },
    signer: 'creator'
  },
  {
    name: '69-address-collections-contract',
    description: 'GET /{address}/collections signed by that address, filtered by a contract address',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      contract_address: '0x5eed000000000000000000000000000000000103'
    },
    signer: 'creator'
  },
  {
    name: '70-address-collections-search',
    description: 'GET /{address}/collections signed by that address, searching by name, sorted by name',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      q: 'pending',
      sort: 'NAME_ASC'
    },
    signer: 'creator'
  },
  {
    name: '71-address-collections-manager',
    description: 'GET /{address}/collections signed by that address, an on-chain manager',
    method: 'GET',
    path: '/v1/0x1257bf35fafebbfc08ea47903e1f7c36ecda48c0/collections',
    signer: 'manager'
  },
  {
    name: '72-address-collections-minter',
    description: 'GET /{address}/collections signed by that address, an on-chain minter',
    method: 'GET',
    path: '/v1/0xc1eba15006e9d7f629bcad7a7dd41285f012b949/collections',
    signer: 'minter'
  },
  {
    name: '73-address-collections-new-creator',
    description: 'GET /{address}/collections signed by that address, the chain creator of a collection another wallet stored',
    method: 'GET',
    path: '/v1/0xcd6884d4258edc833f31ac773e24affe6a7d3154/collections',
    signer: 'stranger'
  },
  {
    name: '74-address-collections-other',
    description: 'GET /{address}/collections signed by another wallet',
    method: 'GET',
    path: '/v1/0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e/collections',
    signer: 'creator'
  },
  {
    name: '75-address-collections-invalid-status',
    description: 'GET /{address}/collections signed by that address, with an unknown status',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      status: 'bogus'
    },
    signer: 'creator'
  },
  {
    name: '76-address-collections-invalid-contract',
    description: 'GET /{address}/collections signed by that address, with a contract address that is not an address',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      contract_address: 'nope'
    },
    signer: 'creator'
  },
  {
    name: '77-address-collections-unsigned',
    description: 'GET /{address}/collections without a signature',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections'
  },
  {
    name: '78-collection-minter',
    description: 'GET /collections/{id} signed by an on-chain minter of an approved collection',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000103',
    signer: 'minter'
  },
  {
    name: '79-collection-published-committee',
    description: 'GET /collections/{id} signed by a committee member who holds no other role on a collection on chain',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000102',
    signer: 'committee'
  },
  {
    name: '80-collection-third-party-manager',
    description: 'GET /collections/{id} signed by a manager of the provider of the third-party collection',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000105',
    signer: 'creator'
  },
  {
    name: '81-collection-third-party-stranger',
    description: 'GET /collections/{id} signed by a wallet that manages no provider of the third-party collection',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000105',
    signer: 'stranger'
  },
  {
    name: '82-collection-missing',
    description: 'GET /collections/{id} signed by the owner for an id no collection has',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-0000000000ff',
    signer: 'owner'
  },
  {
    name: '83-collection-malformed-id',
    description: 'GET /collections/{id} signed by the owner with an id that is not a UUID',
    method: 'GET',
    path: '/v1/collections/not-a-uuid',
    signer: 'owner'
  },
  {
    name: '84-collection-approved-stranger',
    description: 'GET /collections/{id} signed by a wallet that holds no role on an approved collection',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000103',
    signer: 'stranger'
  },
  {
    name: '85-collection-save-create',
    description: 'PUT /collections/{id} signed by the owner, creating a draft with initialize data that names it as the creator',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-0000000000f1',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-0000000000f1',
        name: 'Conformance created',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: OWNER_INITIALIZE
    },
    volatile: [
      'data.created_at',
      'data.updated_at'
    ]
  },
  {
    name: '86-collection-save-rename',
    description: 'PUT /collections/{id} signed by the owner, renaming its draft',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000001',
        name: 'Conformance renamed',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: '0x72d18e0bfa67ba8dce8e9463c8953dce9bd7e4bd88f9149e54b63f974fabefea',
        contract_address: '0x00cc1666ebe7da593c2a77d5bb9686e923f44e26',
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: OWNER_INITIALIZE
    },
    volatile: [
      'data.updated_at'
    ]
  },
  {
    name: '87-collection-save-name-in-use',
    description: 'PUT /collections/{id} signed by the owner, creating a draft whose name another collection has in another case',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-0000000000f2',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-0000000000f2',
        name: 'LISTING DRAFT',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: OWNER_INITIALIZE
    }
  },
  {
    name: '88-collection-save-ids-differ',
    description: 'PUT /collections/{id} signed by the owner, with a body id that is not the path id',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-0000000000f3',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-0000000000f4',
        name: 'Conformance ids',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: OWNER_INITIALIZE
    }
  },
  {
    name: '89-collection-save-without-data',
    description: 'PUT /collections/{id} signed by the owner, creating a draft without data',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-0000000000f5',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-0000000000f5',
        name: 'Conformance without data',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      }
    }
  },
  {
    name: '90-collection-save-claims-publication',
    description: 'PUT /collections/{id} signed by the owner, creating a draft that claims to be published',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-0000000000f6',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-0000000000f6',
        name: 'Conformance claims',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: true,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: OWNER_INITIALIZE
    }
  },
  {
    name: '91-collection-save-published',
    description: 'PUT /collections/{id} signed by the owner of a collection on chain',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000006',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000006',
        name: 'Chain renamed',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: OWNER_INITIALIZE
    }
  },
  {
    name: '92-collection-save-extra-field',
    description: 'PUT /collections/{id} signed by the owner, creating a draft whose body has a field the schema does not name',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-0000000000f7',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-0000000000f7',
        name: 'Conformance extra',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: [],
        forum_post: 'x'
      },
      data: OWNER_INITIALIZE
    },
    volatile: [
      'data.created_at',
      'data.updated_at'
    ]
  },
  {
    name: '93-collection-save-foreign-creator',
    description: 'PUT /collections/{id} signed by the owner, creating a draft with initialize data that names another wallet as the creator',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-0000000000f8',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-0000000000f8',
        name: 'Conformance foreign',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: STRANGER_INITIALIZE
    },
    volatile: [
      'data.created_at',
      'data.updated_at'
    ]
  },
  {
    name: '94-collection-save-third-party-create',
    description: 'PUT /collections/{id} signed by a manager of the provider, creating a third-party collection',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-0000000000f9',
    signer: 'creator',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-0000000000f9',
        name: 'Listing created',
        eth_address: '0x05bb381a642d932037707c4a626f93fb009696b2',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: [],
        urn: 'urn:decentraland:matic:collections-thirdparty:listing:listing-created'
      },
      data: ''
    },
    volatile: [
      'data.created_at',
      'data.updated_at'
    ]
  },
  {
    name: '95-collection-save-third-party-urn-in-use',
    description: 'PUT /collections/{id} signed by a manager of the provider, creating a third-party collection with a URN another collection has',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-0000000000fa',
    signer: 'creator',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-0000000000fa',
        name: 'Listing duplicate',
        eth_address: '0x05bb381a642d932037707c4a626f93fb009696b2',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: [],
        urn: 'urn:decentraland:matic:collections-thirdparty:listing:listing-linked'
      },
      data: ''
    }
  },
  {
    name: '96-collection-save-third-party-rename-urn',
    description: 'PUT /collections/{id} signed by a manager of the provider, changing the URN of an unpublished third-party collection',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000105',
    signer: 'creator',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000105',
        name: 'Listing linked',
        eth_address: '0x05bb381a642d932037707c4a626f93fb009696b2',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: [],
        urn: 'urn:decentraland:matic:collections-thirdparty:listing:listing-renamed',
        linked_contract_address: '0x5eed00000000000000000000000000000000c0de',
        linked_contract_network: 'mainnet'
      },
      data: ''
    },
    volatile: [
      'data.updated_at'
    ]
  },
  {
    name: '97-collection-save-third-party-as-standard',
    description: 'PUT /collections/{id} signed by the owner, saving a third-party collection without its URN',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000008',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000008',
        name: 'Conformance linked A',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: OWNER_INITIALIZE
    }
  },
  {
    name: '98-collection-save-standard-as-third-party',
    description: 'PUT /collections/{id} signed by the owner, saving its standard draft with a third-party URN',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000001',
        name: 'Conformance renamed',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: [],
        urn: 'urn:decentraland:matic:collections-thirdparty:conformance:converted'
      },
      data: OWNER_INITIALIZE
    }
  },
  {
    name: '99-collection-save-third-party-provider-change',
    description: 'PUT /collections/{id} signed by a manager of the provider, moving its collection to another provider',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000105',
    signer: 'creator',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000105',
        name: 'Listing linked',
        eth_address: '0x05bb381a642d932037707c4a626f93fb009696b2',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: [],
        urn: 'urn:decentraland:matic:collections-thirdparty:conformance:listing-renamed'
      },
      data: ''
    }
  }
,
  {
    name: '100-address-collections-published-only',
    description: 'GET /{address}/collections signed by that address, filtered to published collections',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      is_published: 'true'
    },
    signer: 'creator'
  },
  {
    name: '101-address-collections-published-search',
    description: 'GET /{address}/collections signed by that address, filtered to published collections and searching by name',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      is_published: 'true',
      q: 'approved'
    },
    signer: 'creator'
  },
  {
    name: '102-address-collections-published-no-match',
    description: 'GET /{address}/collections signed by that address, filtered to published collections with a search nothing matches',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      is_published: 'true',
      q: 'zzz'
    },
    signer: 'creator'
  },
  {
    name: '103-address-collections-published-page',
    description: 'GET /{address}/collections signed by that address, filtered to published collections, paginated',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      is_published: 'true',
      page: '1',
      limit: '1'
    },
    signer: 'creator'
  },
  {
    name: '104-address-collections-unpublished-search',
    description: 'GET /{address}/collections signed by that address, filtered to unpublished collections and searching by name',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      is_published: 'false',
      q: 'draft'
    },
    signer: 'creator'
  },
  {
    name: '105-address-collections-standard',
    description: 'GET /{address}/collections signed by that address, filtered to standard collections',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      type: 'standard'
    },
    signer: 'creator'
  },
  {
    name: '106-address-collections-page-two',
    description: 'GET /{address}/collections signed by that address, filtered to standard collections, the second page of two',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      type: 'standard',
      page: '2',
      limit: '2'
    },
    signer: 'creator'
  },
  {
    name: '107-address-collections-oldest-first',
    description: 'GET /{address}/collections signed by that address, filtered to standard collections, oldest first',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      type: 'standard',
      sort: 'CREATED_AT_ASC'
    },
    signer: 'creator'
  },
  {
    name: '108-address-collections-none',
    description: 'GET /{address}/collections signed by that address, which holds no collection',
    method: 'GET',
    path: '/v1/0x01eba3f0e992f53debaf51ed3738dc2f23e09aee/collections',
    signer: 'committee'
  },
  {
    name: '109-address-collections-committee-other',
    description: 'GET /{address}/collections signed by a committee member for another address',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    signer: 'committee'
  },
  {
    name: '110-collections-committee-curation-page',
    description: 'GET /collections signed by a committee member, as the curation page asks for its first page',
    method: 'GET',
    path: '/v1/collections',
    query: {
      page: '1',
      limit: '12',
      is_published: 'true',
      sort: 'MOST_RELEVANT'
    },
    signer: 'committee'
  },
  {
    name: '111-collections-committee-curation-page-two',
    description: 'GET /collections signed by a committee member, as the curation page asks for a later page',
    method: 'GET',
    path: '/v1/collections',
    query: {
      page: '2',
      limit: '2',
      is_published: 'true',
      sort: 'MOST_RELEVANT'
    },
    signer: 'committee'
  },
  {
    name: '112-collections-committee-third-party',
    description: 'GET /collections signed by a committee member, filtered to third-party collections',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'conformance linked',
      type: 'third_party'
    },
    signer: 'committee'
  },
  {
    name: '113-collections-committee-review-new',
    description: 'GET /collections signed by a committee member, filtered to reviews of new collections',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      type: 'standard',
      review_type: 'new'
    },
    signer: 'committee'
  },
  {
    name: '114-collections-committee-sort-name-asc',
    description: 'GET /collections signed by a committee member, standard collections sorted by NAME_ASC',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      type: 'standard',
      sort: 'NAME_ASC'
    },
    signer: 'committee'
  },
  {
    name: '115-collections-committee-sort-created-at-asc',
    description: 'GET /collections signed by a committee member, standard collections sorted by CREATED_AT_ASC',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      type: 'standard',
      sort: 'CREATED_AT_ASC'
    },
    signer: 'committee'
  },
  {
    name: '116-collections-committee-sort-created-at-desc',
    description: 'GET /collections signed by a committee member, standard collections sorted by CREATED_AT_DESC',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      type: 'standard',
      sort: 'CREATED_AT_DESC'
    },
    signer: 'committee'
  },
  {
    name: '117-collections-committee-sort-updated-at-asc',
    description: 'GET /collections signed by a committee member, standard collections sorted by UPDATED_AT_ASC',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      type: 'standard',
      sort: 'UPDATED_AT_ASC'
    },
    signer: 'committee'
  },
  {
    name: '118-collections-committee-sort-updated-at-desc',
    description: 'GET /collections signed by a committee member, standard collections sorted by UPDATED_AT_DESC',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      type: 'standard',
      sort: 'UPDATED_AT_DESC'
    },
    signer: 'committee'
  },
  {
    name: '119-collections-committee-sort-last-activity-asc',
    description: 'GET /collections signed by a committee member, standard collections sorted by LAST_ACTIVITY_ASC',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      type: 'standard',
      sort: 'LAST_ACTIVITY_ASC'
    },
    signer: 'committee'
  },
  {
    name: '120-collections-committee-sort-requested-at-desc',
    description: 'GET /collections signed by a committee member, standard collections sorted by REQUESTED_AT_DESC',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      type: 'standard',
      sort: 'REQUESTED_AT_DESC'
    },
    signer: 'committee'
  },
  {
    name: '121-collections-committee-assignee',
    description: 'GET /collections signed by a committee member, filtered to a curator nobody assigned',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      assignee: '0x01eba3f0e992f53debaf51ed3738dc2f23e09aee'
    },
    signer: 'committee'
  },
  {
    name: '122-collections-committee-rejected',
    description: 'GET /collections signed by a committee member, filtered to rejected, under review and auto-reviewing collections',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      status: [
        'rejected',
        'under_review',
        'auto_reviewing'
      ]
    },
    signer: 'committee'
  },
  {
    name: '123-collections-public-search-page',
    description: 'GET /collections unsigned, searching published collections, paginated',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      is_published: 'true',
      page: '1',
      limit: '1'
    }
  },
  {
    name: '124-collections-public-search-tag',
    description: 'GET /collections unsigned, searching published collections with an item tag',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'listing',
      is_published: 'true',
      tag: 'hat'
    }
  },
  {
    name: '125-collections-public-search-type',
    description: 'GET /collections unsigned, searching published standard collections',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'chain',
      is_published: 'true',
      type: 'standard'
    }
  },
  {
    name: '126-collection-new-chain-creator',
    description: 'GET /collections/{id} signed by the chain creator of a collection another wallet stored',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000106',
    signer: 'stranger'
  },
  {
    name: '127-collection-stored-owner-after-transfer',
    description: 'GET /collections/{id} signed by the stored owner of a collection whose chain creator is another wallet',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000106',
    signer: 'creator'
  },
  {
    name: '128-collection-third-party-committee',
    description: 'GET /collections/{id} signed by a committee member who holds no other role on a third-party draft',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000105',
    signer: 'committee',
    volatile: [
      'data.updated_at'
    ]
  },
  {
    name: '129-collection-admin',
    description: 'GET /collections/{id} signed by an ADMIN_ADDRESSES wallet on a collection on chain',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000006',
    signer: 'admin'
  },
  {
    name: '130-collection-save-draft-managers',
    description: 'PUT /collections/{id} signed by the owner, saving its draft with a manager in the body',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000001',
        name: 'Conformance renamed',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: [
          '0x1257bf35fafebbfc08ea47903e1f7c36ecda48c0'
        ]
      },
      data: OWNER_INITIALIZE
    },
    volatile: [
      'data.updated_at'
    ]
  },
  {
    name: '131-collection-body-manager',
    description: 'GET /collections/{id} signed by a wallet the draft names as manager in its body only',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    signer: 'manager'
  },
  {
    name: '132-collection-save-unsigned',
    description: 'PUT /collections/{id} without a signature, with a valid body',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000201',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000201',
        name: 'Conformance unsigned',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: OWNER_INITIALIZE
    }
  },
  {
    name: '133-collection-save-third-party-stranger',
    description: 'PUT /collections/{id} signed by a wallet that manages no provider, saving an existing third-party collection',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000105',
    signer: 'stranger',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000105',
        name: 'Listing linked',
        eth_address: '0xcd6884d4258edc833f31ac773e24affe6a7d3154',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: [],
        urn: 'urn:decentraland:matic:collections-thirdparty:listing:listing-renamed'
      },
      data: ''
    }
  },
  {
    name: '134-collection-save-name-of-third-party',
    description: 'PUT /collections/{id} signed by the owner, creating a draft named as a third-party collection',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000202',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000202',
        name: 'Listing linked',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: OWNER_INITIALIZE
    },
    volatile: [
      'data.created_at',
      'data.updated_at'
    ]
  },
  {
    name: '135-collection-save-name-case-and-spaces',
    description: 'PUT /collections/{id} signed by the owner, creating a draft whose name differs from one in use only by case and spaces',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000203',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000203',
        name: ' CONFORMANCE RENAMED ',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: []
      },
      data: OWNER_INITIALIZE
    }
  },
  {
    name: '136-collection-save-approval-claim',
    description: 'PUT /collections/{id} signed by the owner, creating a draft that claims approval only',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000204',
    signer: 'owner',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000204',
        name: 'Conformance approval',
        eth_address: '0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: true,
        minters: [],
        managers: []
      },
      data: OWNER_INITIALIZE
    }
  },
  {
    name: '137-collection-save-linked-contract-taken',
    description: 'PUT /collections/{id} signed by a manager of the provider, creating a collection linked to a contract another collection of the provider links',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000205',
    signer: 'creator',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000205',
        name: 'Listing duplicate link',
        eth_address: '0x05bb381a642d932037707c4a626f93fb009696b2',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: [],
        urn: 'urn:decentraland:matic:collections-thirdparty:listing:listing-duplicate',
        linked_contract_address: '0x5eed00000000000000000000000000000000c0de',
        linked_contract_network: 'mainnet'
      },
      data: ''
    }
  },
  {
    name: '138-collection-save-linked-contract-cleared',
    description: 'PUT /collections/{id} signed by a manager of the provider, clearing the linked contract of its collection',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000105',
    signer: 'creator',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000105',
        name: 'Listing linked',
        eth_address: '0x05bb381a642d932037707c4a626f93fb009696b2',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: [],
        urn: 'urn:decentraland:matic:collections-thirdparty:listing:listing-renamed',
        linked_contract_address: null,
        linked_contract_network: null
      },
      data: ''
    }
  },
  {
    name: '139-collection-save-registered-provider-stranger',
    description: 'PUT /collections/{id} signed by a wallet that manages no provider, creating a collection under a provider the registry lists',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000206',
    signer: 'stranger',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000206',
        name: 'Registered summer',
        eth_address: '0xcd6884d4258edc833f31ac773e24affe6a7d3154',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: [],
        urn: 'urn:decentraland:matic:collections-thirdparty:registered:summer'
      },
      data: ''
    }
  },
  {
    name: '140-collection-save-registered-provider-manager',
    description: 'PUT /collections/{id} signed by a manager the registry lists, creating a collection under its provider',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000207',
    signer: 'manager',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000207',
        name: 'Registered winter',
        eth_address: '0x1257bf35fafebbfc08ea47903e1f7c36ecda48c0',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: [],
        urn: 'urn:decentraland:matic:collections-thirdparty:registered:winter'
      },
      data: ''
    },
    volatile: [
      'data.created_at',
      'data.updated_at'
    ]
  },
  {
    name: '141-collection-registered-manager',
    description: 'GET /collections/{id} signed by another manager the registry lists for the provider of a third-party collection',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000207',
    signer: 'minter',
    volatile: [
      'data.created_at',
      'data.updated_at'
    ]
  },
  {
    name: '142-address-collections-registered-manager',
    description: 'GET /{address}/collections signed by that address, a manager the registry lists for a provider',
    method: 'GET',
    path: '/v1/0xc1eba15006e9d7f629bcad7a7dd41285f012b949/collections',
    signer: 'minter',
    volatile: [
      'data.created_at',
      'data.updated_at',
      'data.last_activity_at'
    ]
  },
  {
    name: '143-collections-committee-registered-provider',
    description: 'GET /collections signed by a committee member, searching collections of a provider the registry lists',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'registered'
    },
    signer: 'committee',
    volatile: [
      'data.created_at',
      'data.updated_at',
      'data.last_activity_at'
    ]
  },
  {
    name: '144-collection-save-programmatic-provider',
    description: 'PUT /collections/{id} signed by a manager the registry lists, creating a collection under its programmatic provider',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000208',
    signer: 'manager',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000208',
        name: 'Programmatic spring',
        eth_address: '0x1257bf35fafebbfc08ea47903e1f7c36ecda48c0',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: [],
        urn: 'urn:decentraland:matic:collections-thirdparty:programmatic:spring'
      },
      data: ''
    },
    volatile: [
      'data.created_at',
      'data.updated_at'
    ]
  },
  {
    name: '145-collection-programmatic-provider',
    description: 'GET /collections/{id} signed by the manager of the programmatic provider of a third-party collection',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000208',
    signer: 'manager',
    volatile: [
      'data.created_at',
      'data.updated_at'
    ]
  },
  {
    name: '146-address-collections-programmatic-provider',
    description: 'GET /{address}/collections signed by that address, a manager the registry lists for a programmatic provider',
    method: 'GET',
    path: '/v1/0x1257bf35fafebbfc08ea47903e1f7c36ecda48c0/collections',
    query: {
      type: 'third_party'
    },
    signer: 'manager',
    volatile: [
      'data.created_at',
      'data.updated_at',
      'data.last_activity_at'
    ]
  },
  {
    name: '147-collections-committee-programmatic-provider',
    description: 'GET /collections signed by a committee member, searching collections of a programmatic provider',
    method: 'GET',
    path: '/v1/collections',
    query: {
      q: 'spring'
    },
    signer: 'committee',
    volatile: [
      'data.created_at',
      'data.updated_at',
      'data.last_activity_at'
    ]
  },
  {
    name: '148-collection-save-registered-other-manager',
    description: 'PUT /collections/{id} signed by a manager the registry lists, saving a collection another manager of the provider created',
    method: 'PUT',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000207',
    signer: 'minter',
    body: {
      collection: {
        id: 'c011ec71-0000-4000-8000-000000000207',
        name: 'Registered winter',
        eth_address: '0xc1eba15006e9d7f629bcad7a7dd41285f012b949',
        salt: null,
        contract_address: null,
        is_published: false,
        is_approved: false,
        minters: [],
        managers: [],
        urn: 'urn:decentraland:matic:collections-thirdparty:registered:winter'
      },
      data: ''
    },
    volatile: [
      'data.created_at',
      'data.updated_at'
    ]
  },
  {
    name: '149-address-collections-query-address-other',
    description: 'GET /{address}/collections signed by that address, with an address query parameter naming another wallet',
    method: 'GET',
    path: '/v1/0x05bb381a642d932037707c4a626f93fb009696b2/collections',
    query: {
      address: '0xcd6884d4258edc833f31ac773e24affe6a7d3154',
      type: 'standard'
    },
    signer: 'creator'
  },
  {
    name: '150-address-collections-query-address-own',
    description: 'GET /{address}/collections signed by another wallet, with an address query parameter naming the signer',
    method: 'GET',
    path: '/v1/0xcd6884d4258edc833f31ac773e24affe6a7d3154/collections',
    query: {
      address: '0x05bb381a642d932037707c4a626f93fb009696b2',
      type: 'standard'
    },
    signer: 'creator'
  },
  {
    name: '151-collection-query-id',
    description: 'GET /collections/{id} signed by the owner, with an id query parameter naming another collection',
    method: 'GET',
    path: '/v1/collections/c011ec71-0000-4000-8000-000000000001',
    query: {
      id: 'c011ec71-0000-4000-8000-000000000006'
    },
    signer: 'owner',
    volatile: [
      'data.updated_at'
    ]
  },
  ...ITEM_CASES,
  ...SMALL_READ_CASES,
  ...LIFECYCLE_CASES,
  ...THIRD_PARTY_CASES,
  ...CURATION_CASES,
  ...CURATION_ROUTE_CASES,
  ...FORUM_CASES,
  ...THIRD_PARTY_PUBLISH_CASES,
  ...VALIDATION_CASES,
  ...VIDEO_CASES,
  ...REVIEW_CASES,
  ...EDGE_CASES,
  ...LISTING_CASES
]

/** Why `cases` can't run in the order of their numbers: a name without a number, or a number not above the one before. */
function caseOrderErrors(cases) {
  const errors = []
  let previous
  for (const { name } of cases) {
    const number = /^(\d+)-/.exec(name)?.[1]
    if (number === undefined) {
      errors.push(`${name}: a case name starts with its number`)
      continue
    }
    if (previous && Number(number) <= Number(previous.number)) errors.push(`${name} runs after ${previous.name}`)
    previous = { name, number }
  }
  return errors
}

/** The source captures a gap of `ledger` (parity.json) lists that no case of `cases` records. */
function ledgerCaptureErrors(ledger, cases) {
  const recorded = new Set(cases.map(({ name }) => `source/${name}.json`))
  return ['deliberate', 'open'].flatMap(kind =>
    Object.entries(ledger[`${kind}_gaps`] ?? {}).flatMap(([name, gap]) =>
      gap.captures
        .filter(capture => capture.startsWith('source/') && !recorded.has(capture))
        .map(capture => `${kind} gap ${name}: lists ${capture}, which is no capture`)
    )
  )
}

function fail(message) {
  throw Error(message)
}

function decoded(text) {
  try {
    return decodeURIComponent(text)
  } catch {
    return text
  }
}

function passwordOf(databaseUrl) {
  try {
    return new URL(databaseUrl).password
  } catch {
    return /^[^:]+:\/\/[^:@/]*:([^@]*)@/.exec(databaseUrl)?.[1] ?? ''
  }
}

/** The message with the database URL and every spelling of its password replaced; never throws. */
function redact(message, databaseUrl) {
  const url = typeof databaseUrl === 'string' ? databaseUrl : ''
  const password = url ? passwordOf(url) : ''
  const secrets = [url, password, decoded(password)].filter(Boolean)
  return secrets.reduce((text, secret) => text.split(secret).join('<redacted>'), String(message))
}

function databaseUrl(base, name) {
  const url = new URL(base)
  url.pathname = '/' + name
  return url.toString()
}

/** Creates a scratch database, runs `body` with its URL, and drops it afterwards, on failure and on SIGINT or SIGTERM. */
async function withScratchDatabase(admin, adminUrl, body) {
  const name = `builder_capture_${process.pid}_${crypto.randomBytes(6).toString('hex')}`
  let dropping
  const drop = () => (dropping ??= admin.query(`DROP DATABASE IF EXISTS "${name}" WITH (FORCE)`).finally(() => admin.end()))
  const interrupted = signal => drop().finally(() => process.exit(128 + os.constants.signals[signal]))
  activeDrop = drop
  process.on('SIGINT', interrupted)
  process.on('SIGTERM', interrupted)
  try {
    await admin.query(`CREATE DATABASE "${name}"`)
    return await body(databaseUrl(adminUrl, name))
  } finally {
    process.off('SIGINT', interrupted)
    process.off('SIGTERM', interrupted)
    await drop()
    activeDrop = undefined
  }
}

/** Publishes `fixtures` ([name, text] pairs) into `output` only when `problems` is empty, staging them beside it first. */
function finish(output, fixtures, problems) {
  if (problems.length) fail(`upstream reached a service the capture does not fake: ${[...new Set(problems)].join(', ')}`)
  const target = path.resolve(output)
  const staging = fs.mkdtempSync(path.join(path.dirname(target), '.builder-capture-'))
  try {
    for (const [name, text] of fixtures) fs.writeFileSync(path.join(staging, name), text)
    fs.mkdirSync(target, { recursive: true })
    for (const [name] of fixtures) fs.renameSync(path.join(staging, name), path.join(target, name))
  } finally {
    fs.rmSync(staging, { recursive: true, force: true })
  }
}

function assertLockedInstall(upstream) {
  const read = file => JSON.parse(fs.readFileSync(path.join(upstream, file), 'utf8')).packages
  let locked, installed
  try {
    locked = read('package-lock.json')
    installed = read('node_modules/.package-lock.json')
  } catch {
    fail('Install the upstream dependencies with npm ci before capturing')
  }
  const differing = Object.entries(locked)
    .filter(([name, entry]) => name && !entry.optional)
    .filter(([name, entry]) => installed[name]?.version !== entry.version || installed[name]?.integrity !== entry.integrity)
    .map(([name]) => name)
  const extra = Object.keys(installed).filter(name => !locked[name])
  if (differing.length || extra.length) {
    fail(`node_modules differs from package-lock.json (${[...differing, ...extra].slice(0, 5).join(', ')}); install with npm ci`)
  }
}

function freezeClock() {
  const fixed = Date.parse(CLOCK)
  const RealDate = Date
  class FixedDate extends RealDate {
    constructor(...args) {
      if (args.length === 0) super(fixed)
      else super(...args)
    }
    static now() { return fixed }
  }
  globalThis.Date = FixedDate
}

function readSeed() {
  const seed = JSON.parse(fs.readFileSync(path.join(FIXTURES, 'seed.json'), 'utf8'))
  const unknown = Object.keys(seed).filter(key => !SEED_KEYS.includes(key))
  if (unknown.length) fail(`seed.json fields the capture does not apply: ${unknown.join(', ')}`)
  return seed
}

function wallets(req, seed) {
  const { ethers } = req('ethers')
  const key = label => ethers.utils.keccak256(ethers.utils.toUtf8Bytes(`dcl-one builder conformance ${label}`))
  const out = {}
  for (const [role, recorded] of Object.entries(seed.wallets)) {
    const root = new ethers.Wallet(key(role))
    const ephemeral = new ethers.Wallet(key(`${role} ephemeral`))
    const address = root.address.toLowerCase()
    if (address !== recorded) fail(`seed.json wallet ${role} must be ${address}`)
    out[role] = { root, ephemeral, address }
  }
  return out
}

/**
 * The headers a client signs for `item`: the timestamped payload over its method and path with
 * `item.metadata` (`{}` by default), with the one defect `item.sign` names: `offset` moves the
 * timestamp, `path` or `method` signs another one, `sentMetadata` sends other metadata than was
 * signed, and `untimestamped` signs `method:path` below `/v1` alone, as pre-ADR-44 clients do,
 * sending the timestamp and metadata headers too with `withHeaders`.
 */
async function signedHeaders(req, wallet, item) {
  const { Authenticator } = req('@dcl/crypto')
  const identity = await Authenticator.initializeAuthChain(
    wallet.address,
    { address: wallet.ephemeral.address, privateKey: wallet.ephemeral.privateKey, publicKey: wallet.ephemeral.publicKey },
    60,
    message => wallet.root.signMessage(message)
  )
  const sign = item.sign || {}
  const chainHeaders = payload => Object.fromEntries(
    Authenticator.signPayload(identity, payload).map((link, index) => [`x-identity-auth-chain-${index}`, JSON.stringify(link)])
  )
  const method = sign.method || item.method
  if (sign.untimestamped) {
    const headers = chainHeaders(`${method}:${item.path.replace(/^\/v1/, '')}`.toLowerCase())
    return sign.withHeaders ? { ...headers, 'x-identity-timestamp': String(Date.now()), 'x-identity-metadata': '{}' } : headers
  }
  const timestamp = String(Date.now() + (sign.offset || 0))
  const metadata = item.metadata || '{}'
  const payload = [method, sign.path || item.path, timestamp].join(':').toLowerCase() + ':' + metadata
  return {
    ...chainHeaders(payload),
    'x-identity-timestamp': timestamp,
    'x-identity-metadata': sign.sentMetadata || metadata
  }
}

/** The collections subgraph's rows for `seed.chain`, in the shape upstream's fragments select. */
function chainRows(seed, signers, req) {
  const addresses = roles => roles.map(role => signers[role].address)
  const accounts = addresses(seed.chain.committee).map(address => ({ id: address, address, isCommitteeMember: true }))
  const collections = seed.chain.collections.map(c => ({
    __typename: 'Collection',
    id: c.contract_address,
    creator: signers[c.creator].address,
    owner: signers[c.owner].address,
    name: c.name,
    isApproved: c.is_approved,
    isCompleted: true,
    minters: addresses(c.minters),
    managers: addresses(c.managers),
    reviewedAt: String(c.reviewed_at),
    updatedAt: String(c.updated_at),
    createdAt: String(c.created_at)
  }))
  const items = seed.chain.items.map(i => {
    const collection = collections.find(c => c.id === i.contract_address)
    return {
      __typename: 'Item',
      id: `${i.contract_address}-${i.blockchain_id}`,
      blockchainId: i.blockchain_id,
      urn: `urn:decentraland:matic:collections-v2:${i.contract_address}:${i.blockchain_id}`,
      totalSupply: i.total_supply,
      price: i.price,
      beneficiary: signers[i.beneficiary].address,
      managers: [],
      minters: [],
      contentHash: i.content_hash,
      collection: {
        __typename: 'Collection',
        id: collection.id,
        creator: collection.creator,
        owner: collection.owner,
        name: collection.name,
        isApproved: collection.isApproved,
        minters: collection.minters,
        managers: collection.managers
      },
      metadata: {
        __typename: 'Metadata',
        wearable: {
          __typename: 'Wearable',
          name: i.name,
          description: '',
          category: i.category,
          rarity: i.rarity,
          bodyShapes: ['BaseMale']
        }
      }
    }
  })
  for (const collection of collections) collection.items = items.filter(item => item.collection.id === collection.id)
  const registry = registryRows(seed.registry, role => signers[role].address)
  const { ethers } = req('ethers')
  const receipts = receiptRows((seed.receipts ?? []).map(r => {
    const { domain, types, value } = chequeTypedData(r.provider, r.qty, r.salt)
    return { id: ethers.utils._TypedDataEncoder.hash(domain, types, value).toLowerCase(), qty: r.qty, signer: signers[r.signer].address }
  }))
  return { accounts, collections, items, registry, receipts, entities: seed.entities ?? [], rarities: rarityRows(seed.chain.rarities), oracle: seed.chain.rarities }
}

/**
 * The third-party subgraph's rows for the seeded `registry`: approved unless a provider says `approved: false`,
 * with no `__typename`, which The Graph answers only when a query selects it.
 */
function registryRows(registry, address) {
  return registry.map(provider => ({
    id: provider.id,
    root: null,
    managers: provider.managers.map(address),
    maxItems: provider.max_items ?? '100',
    isApproved: provider.approved ?? true,
    isProgrammatic: !!provider.programmatic,
    metadata: { type: 'third_party_v2', thirdParty: { name: provider.name, description: '', contracts: [] } }
  }))
}

/**
 * The content server's answer to `POST /content/entities/active {pointers}` from the seeded `entities`: each active
 * entity at a pointer asked for, pointers compared lowercased as the content server stores them.
 */
function activeEntities(entities, bytes) {
  const asked = new Set((JSON.parse(bytes.toString('utf8') || '{}').pointers ?? []).map(pointer => String(pointer).toLowerCase()))
  return entities
    .filter(entity => asked.has(entity.pointer.toLowerCase()))
    .map(entity => ({ version: 'v3', id: entity.entity_id, type: entity.type, pointers: [entity.pointer.toLowerCase()], timestamp: 1767225600000, content: [], metadata: entity.metadata }))
}

/** The third-party subgraph's `receipts` of the cheques the registry consumed, each `{id, qty, signer}`. */
function receiptRows(receipts) {
  return receipts.map(r => ({ id: r.id, qty: String(r.qty), signer: r.signer }))
}

/** The Polygon Third Party Registry's EIP-712 typed data a manager signs to consume `qty` slots of `provider` under `salt`. */
function chequeTypedData(provider, qty, salt) {
  return {
    domain: {
      name: 'Decentraland Third Party Registry',
      version: '1',
      verifyingContract: '0x1C436C1EFb4608dFfDC8bace99d2B03c314f3348',
      salt: `0x${'00'.repeat(31)}89`
    },
    types: { ConsumeSlots: [{ name: 'thirdPartyId', type: 'string' }, { name: 'qty', type: 'uint256' }, { name: 'salt', type: 'bytes32' }] },
    value: { thirdPartyId: provider, qty, salt }
  }
}

/**
 * `item.body` with the cheque `item.cheque` describes: `signer` signs `qty` slots of `provider` under
 * `salt`; `sentQty` sends another quantity than was signed.
 */
async function withCheque(item, signers) {
  if (!item.cheque) return item.body
  const { signer, provider, qty, salt, sentQty } = item.cheque
  const { domain, types, value } = chequeTypedData(provider, qty, salt)
  const signature = await signers[signer].root._signTypedData(domain, types, value)
  return { ...item.body, cheque: { qty: sentQty ?? qty, salt, signature } }
}

/** The collections subgraph's `rarities` for `seed.chain.rarities`, with the fields upstream selects. */
function rarityRows(rarities) {
  return rarities.map(r => ({ id: r.name, name: r.name, price: r.price, maxSupply: r.max_supply }))
}

/** One 32-byte ABI word of `value` (a hex string or a decimal string of a uint256). */
function word(value) {
  return BigInt(value).toString(16).padStart(64, '0')
}

/**
 * The rarities-with-oracle contract's answer to an `eth_call` of `getRarityByName(string)`:
 * the seeded rarity `(name, maxSupply, mana_price)` ABI-encoded, a revert for a name it doesn't
 * hold, and undefined for any other call.
 */
function oracleAnswer(params, rarities) {
  const [call] = params
  const data = String(call?.data || '').replace(/^0x/, '')
  if (String(call?.to || '').toLowerCase() !== RARITIES_WITH_ORACLE || !data.startsWith(GET_RARITY_BY_NAME) || data.length < 8 + 128) return undefined
  const length = Number(BigInt(`0x${data.slice(8 + 64, 8 + 128)}`))
  const name = Buffer.from(data.slice(8 + 128, 8 + 128 + length * 2), 'hex').toString('utf8')
  const rarity = rarities.find(r => r.name === name)
  if (!rarity) return { error: { code: 3, message: 'execution reverted' } }
  const bytes = Buffer.from(name, 'utf8')
  const padded = bytes.toString('hex').padEnd(Math.ceil(bytes.length / 32) * 64, '0')
  return { result: `0x${word('0x20')}${word('0x60')}${word(rarity.max_supply)}${word(rarity.mana_price)}${word(bytes.length)}${padded}` }
}

/** The value of a GraphQL argument node, with its variables substituted. */
function argumentValue(node, variables) {
  if (node.kind === 'Variable') return variables[node.name.value]
  if (node.kind === 'ListValue') return node.values.map(v => argumentValue(v, variables))
  if (node.kind === 'ObjectValue') return Object.fromEntries(node.fields.map(f => [f.name.value, argumentValue(f.value, variables)]))
  if (node.kind === 'NullValue') return null
  return node.value
}

/** The rows of `field` its `where` argument selects; a filter the fake can't apply is reported. */
function selected(field, rows, variables, unfaked) {
  const arg = name => {
    const node = (field.arguments || []).find(a => a.name.value === name)
    return node ? argumentValue(node.value, variables) : undefined
  }
  const where = arg('where') || {}
  const lower = value => String(value).toLowerCase()
  const containsAll = (list, values, fold) => values.every(v => (list || []).map(fold).includes(fold(v)))
  const matched = rows.filter(row => Object.entries(where).every(([key, value]) => {
    if (value === undefined || value === null) return true
    if (!FAKED_WHERE_KEYS.has(key)) {
      unfaked.push(`graph ${field.name.value} where ${key}`)
      return false
    }
    if (key === 'id') return row.id === value
    if (key === 'id_in') return value.includes(row.id)
    if (key === 'id_gt') return row.id > value
    if (key === 'isCommitteeMember') return row.isCommitteeMember === value
    if (key === 'creator') return row.creator === value
    if (key === 'managers_contains') return containsAll(row.managers, value, String)
    if (key === 'managers_contains_nocase') return containsAll(row.managers, value, lower)
    if (key === 'minters_contains') return containsAll(row.minters, value, String)
    return row.collection?.id === value
  }))
  const orderBy = arg('orderBy') || 'id'
  const ordered = [...matched].sort((a, b) => (a[orderBy] < b[orderBy] ? -1 : a[orderBy] > b[orderBy] ? 1 : 0))
  if (arg('orderDirection') === 'desc') ordered.reverse()
  const skip = Number(arg('skip') || 0)
  const first = Number(arg('first') ?? 100)
  if (first > 1000) throw new Error('The `first` argument must be between 0 and 1000')
  if (skip > 5000) throw new Error('The `skip` argument must be between 0 and 5000')
  return ordered.slice(skip, skip + first)
}

/**
 * The fake S3's answer to one request of upstream's AWS SDK: `PutObject`, `CopyObject` (a PUT
 * naming `x-amz-copy-source`) and `DeleteObject` on the `conformance` bucket, kept in `objects`,
 * and the public `GET` of an object upstream's `exists` probe sends (404 for a key it doesn't
 * hold); undefined for anything else.
 */
function s3Answer(method, url, headers, bytes, objects) {
  const key = decodeURIComponent(url.replace(/^\/s3\/conformance\//, '').split('?')[0])
  if (method === 'PUT' && headers['x-amz-copy-source']) {
    const source = decodeURIComponent(headers['x-amz-copy-source'].replace(/^\/?conformance\//, ''))
    objects.set(key, objects.get(source) || Buffer.alloc(0))
    const body = '<?xml version="1.0" encoding="UTF-8"?><CopyObjectResult><ETag>"conformance"</ETag><LastModified>2026-01-01T00:00:00.000Z</LastModified></CopyObjectResult>'
    return { status: 200, headers: { 'content-type': 'application/xml' }, body }
  }
  if (method === 'PUT') {
    objects.set(key, bytes)
    return { status: 200, headers: { etag: '"conformance"' }, body: '' }
  }
  if (method === 'DELETE') {
    objects.delete(key)
    return { status: 204, headers: {}, body: '' }
  }
  if (method === 'GET') {
    return objects.has(key) ? { status: 200, headers: {}, body: objects.get(key) } : { status: 404, headers: {}, body: '' }
  }
  return undefined
}

/** The token upstream sends the fake warehouse as `x-token`. */
const WAREHOUSE_TOKEN = 'example-warehouse-token'

/**
 * The fake warehouse's answer to one request: a `POST /warehouse` with the configured `x-token` and a
 * JSON body is recorded in `received` and answered 200; a wrong or missing token 401, a body that isn't
 * JSON 400; undefined for anything else.
 */
function warehouseAnswer(method, url, headers, bytes, received) {
  if (method !== 'POST' || url !== '/warehouse') return undefined
  if (headers['x-token'] !== WAREHOUSE_TOKEN) return { status: 401, body: { error: 'Unauthorized' } }
  try {
    received.push(JSON.parse(bytes.toString('utf8')))
  } catch {
    return { status: 400, body: { error: 'Invalid body' } }
  }
  return { status: 200, body: {} }
}

/** The bot token upstream posts to the fake Slack with. */
const SLACK_TOKEN = 'xoxb-example123'
/** The curators' channel upstream posts to. */
const SLACK_CHANNEL = 'curation-example'
const SLACK_TS_SECONDS = Date.parse(CLOCK) / 1000

/**
 * The fake Slack's answer to one request under Slack's origin: `POST /api/chat.postMessage` with the
 * bot token as a bearer, a channel and a text is recorded in `received` and answered as Slack's Web
 * API does, HTTP 200 with `ok: true`; a missing or wrong token, a missing channel or text, or a body
 * that isn't JSON is answered HTTP 200 with `ok: false` and Slack's error code; undefined for any
 * other method or path.
 */
function slackAnswer(method, url, headers, bytes, received) {
  if (method !== 'POST' || url !== '/api/chat.postMessage') return undefined
  const refused = error => ({ status: 200, body: { ok: false, error } })
  const authorization = headers.authorization
  if (!authorization) return refused('not_authed')
  if (authorization !== `Bearer ${SLACK_TOKEN}`) return refused('invalid_auth')
  let message
  try {
    message = JSON.parse(bytes.toString('utf8'))
  } catch {
    return refused('invalid_json')
  }
  if (!message || typeof message.channel !== 'string' || !message.channel) return refused('channel_not_found')
  if (typeof message.text !== 'string' && !Array.isArray(message.blocks)) return refused('no_text')
  received.push(message)
  const ts = `${SLACK_TS_SECONDS}.${String(received.length).padStart(6, '0')}`
  return { status: 200, body: { ok: true, channel: message.channel, ts, message: { type: 'message', text: message.text } } }
}

/**
 * The loopback `http.request` options that send a request upstream addresses to `slack.com` to the
 * fake Slack on `port`, under `/slack`; undefined for any other request.
 */
function slackRoute(options, port) {
  if (!options || typeof options !== 'object' || options.hostname !== 'slack.com') return undefined
  return { protocol: 'http:', hostname: '127.0.0.1', port, path: `/slack${options.path}`, method: options.method, headers: options.headers }
}

/** What a capture records of the Slack messages its case sent: nothing, or `slack` in arrival order. */
function slackRecord(messages) {
  return messages.length ? { slack: messages } : {}
}

/**
 * The fake newsletter service's answer to a request at `url` (below `/newsletter`), recording each
 * subscription it receives; it fails a subscription of `NEWSLETTER_FAILING_EMAIL` with 500.
 */
function newsletterAnswer(method, url, headers, bytes, received) {
  if (method !== 'POST' || url !== `/v2/publications/${NEWSLETTER_PUBLICATION}/subscriptions`) return undefined
  let body
  try {
    body = JSON.parse(bytes.toString('utf8'))
  } catch {
    body = null
  }
  received.push({ method, path: url, authorization: headers.authorization ?? null, accept: headers.accept ?? null, content_type: headers['content-type'] ?? null, body })
  if (body?.email === NEWSLETTER_FAILING_EMAIL) return { status: 500, body: { errors: [{ message: 'Internal error' }] } }
  return { status: 201, body: { data: { id: 'sub_example', email: body?.email, status: 'active' } } }
}

/** What a capture records of the subscriptions its case forwarded: nothing, or `newsletter` in arrival order. */
function newsletterRecord(calls) {
  return calls.length ? { newsletter: calls } : {}
}

/** The forum upstream posts to; the seed's forum links are under it. */
const FORUM_ORIGIN = 'https://forum.example.org'
const FORUM_HOST = 'forum.example.org'
/** The API key upstream sends the fake forum. */
const FORUM_KEY = 'sk-example-forum'
/** The account upstream reads posts as. */
const FORUM_USERNAME = 'builder-example'
/** The category upstream files topics in. */
const FORUM_CATEGORY = '12'

/** Discourse's slug of `title`: lowercase letters and digits, every other run a hyphen, `topic` when empty. */
function forumSlug(title) {
  const slug = title.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '')
  return slug || 'topic'
}

/**
 * The fake forum's state: the seed collections' topics (a topic's id and slug from its link, its first post
 * the collection's `forum_id`), the topics and posts it makes from 1001 and 5001 on, and every call.
 */
function forumState(collections) {
  const forum = { calls: [], topics: [], posts: [], madeTopics: 0, madePosts: 0 }
  for (const { forum_link: link, forum_id: post } of collections) {
    if (!link || !post) continue
    const [, slug, topic] = new URL(link).pathname.match(/^\/t\/([^/]+)\/(\d+)$/)
    forum.topics.push({ id: Number(topic), slug, title: null, posts: [post] })
    forum.posts.push([post, Number(topic)])
  }
  return forum
}

/**
 * The fake forum's answer to one request under the forum's origin, as Discourse answers the calls upstream
 * makes: `POST /posts.json` creates a topic (title, raw, category; a title another topic has, ignoring case,
 * is refused 422) or, with a topic_id, replies in it; `GET /posts/{id}.json` reads a post. Each call is
 * recorded with its path, its `Api-Username` and its JSON body; a missing or wrong `Api-Key` is refused 403;
 * undefined for any other method or path.
 */
function forumAnswer(method, url, headers, bytes, forum) {
  const read = method === 'GET' && url.match(/^\/posts\/(\d+)\.json$/)
  if (!(method === 'POST' && url === '/posts.json') && !read) return undefined
  let body = null
  if (method === 'POST') {
    try { body = JSON.parse(bytes.toString('utf8')) } catch {}
  }
  forum.calls.push({ method, path: url, username: headers['api-username'] ?? null, body })
  const refused = (status, error) => ({ status, body: { action: 'create_post', errors: [error] } })
  const notFound = { status: 404, body: { errors: ['The requested URL or resource could not be found.'], error_type: 'not_found' } }
  if (headers['api-key'] !== FORUM_KEY) return refused(403, 'You are not permitted to view the requested resource.')
  if (read) {
    const post = forum.posts.find(([id]) => id === Number(read[1]))
    return post ? { status: 200, body: { id: post[0], topic_id: post[1], raw: '' } } : notFound
  }
  if (!body || typeof body !== 'object') return refused(422, 'Title can\'t be blank')
  const postId = 5001 + forum.madePosts
  if (body.topic_id !== undefined) {
    const topic = forum.topics.find(t => t.id === body.topic_id)
    if (!topic) return notFound
    topic.posts.push(postId)
    forum.posts.push([postId, topic.id])
    forum.madePosts += 1
    return { status: 200, body: { id: postId, topic_id: topic.id, topic_slug: topic.slug, post_number: topic.posts.length } }
  }
  if (typeof body.title !== 'string' || typeof body.raw !== 'string') return refused(422, 'Title can\'t be blank')
  if (forum.topics.some(t => t.title !== null && t.title.toLowerCase() === body.title.toLowerCase())) return refused(422, 'Title has already been used')
  const topic = { id: 1001 + forum.madeTopics, slug: forumSlug(body.title), title: body.title, posts: [postId] }
  forum.topics.push(topic)
  forum.posts.push([postId, topic.id])
  forum.madeTopics += 1
  forum.madePosts += 1
  return { status: 200, body: { id: postId, topic_id: topic.id, topic_slug: topic.slug, post_number: 1 } }
}

/**
 * The loopback `http.request` options that send a request upstream addresses to the forum's host to the fake
 * forum on `port`, under `/forum`; undefined for any other request.
 */
function forumRoute(options, port) {
  if (!options || typeof options !== 'object' || options.hostname !== FORUM_HOST) return undefined
  return { protocol: 'http:', hostname: '127.0.0.1', port, path: `/forum${options.path}`, method: options.method, headers: options.headers }
}

/** What a capture records of the forum calls its case made: nothing, or `forum` in arrival order. */
function forumRecord(calls) {
  return calls.length ? { forum: calls } : {}
}

/** Sends this process's `https` requests to `slack.com` and the forum's host to the fakes on `port`; answers the undo. */
function routeSlack(port) {
  const request = https.request
  https.request = (options, ...rest) => {
    const routed = slackRoute(options, port) ?? forumRoute(options, port)
    return routed ? http.request(routed, ...rest) : request(options, ...rest)
  }
  return () => { https.request = request }
}

/**
 * Tracks every promise the methods of upstream's `curationNotifications` answer, which upstream
 * fires without awaiting; answers a function that waits until none is pending.
 */
function trackNotices(notifications) {
  const pending = new Set()
  for (const [name, method] of Object.entries(notifications)) {
    if (typeof method !== 'function') continue
    notifications[name] = (...args) => {
      const sent = method.apply(notifications, args)
      pending.add(sent)
      Promise.resolve(sent).finally(() => pending.delete(sent)).catch(() => {})
      return sent
    }
  }
  return async () => {
    while (pending.size) await Promise.allSettled([...pending])
  }
}

/** The secret the validator signs with, and one it doesn't. */
const VALIDATOR_SECRET = 'example-validator-secret-0123456789abcdef'
const WRONG_VALIDATOR_SECRET = 'example-wrong-secret-0123456789abcdefghij'
const SNS_TOPIC = 'arn:aws:sns:us-east-1:000000000000:conformance'

/** builder-server's validator headers for `rawBody`, signed as the case's `validator` says (another secret, a clock offset). */
function validatorHeaders(validator, rawBody) {
  const timestamp = String(Date.now() + (validator.offset || 0))
  const secret = validator.secret === 'wrong' ? WRONG_VALIDATOR_SECRET : VALIDATOR_SECRET
  const signature = crypto.createHmac('sha256', secret).update(`${timestamp}.${rawBody}`).digest('hex')
  return { 'x-wearable-validator-signature': `sha256=${signature}`, 'x-wearable-validator-timestamp': timestamp }
}

/**
 * The events topic's answer to upstream's SNS client (AWS query protocol): a `Publish` of a validation request on
 * the configured topic is recorded in `queued` as its manifest without the random `validationId`; anything else is
 * not faked.
 */
function snsAnswer(method, url, bytes, queued) {
  if (method !== 'POST' || !/^\/sns\/?$/.test(url)) return undefined
  const form = new URLSearchParams(bytes.toString('utf8'))
  if (form.get('Action') !== 'Publish' || form.get('TopicArn') !== SNS_TOPIC) return undefined
  const message = JSON.parse(form.get('Message'))
  const { type, subType, key, metadata } = message
  if (type !== 'builder' || subType !== 'collection-validation-requested' || key !== metadata?.collectionId) return undefined
  const { validationId, ...manifest } = metadata
  if (typeof validationId !== 'string') return undefined
  queued.push(manifest)
  return {
    status: 200,
    body: '<PublishResponse xmlns="http://sns.amazonaws.com/doc/2010-03-31/"><PublishResult><MessageId>c011ec71-0000-4000-8000-0000000005b1</MessageId></PublishResult><ResponseMetadata><RequestId>c011ec71-0000-4000-8000-0000000005b2</RequestId></ResponseMetadata></PublishResponse>'
  }
}

/** What a capture records of the validation requests its case made upstream send: nothing, or `queue` in arrival order. */
function queueRecord(manifests) {
  return manifests.length ? { queue: manifests } : {}
}

/**
 * Lets a capture wait for the validation requests upstream sends in the background. Each request ends in a message
 * on the queue (`ended()` from the fake), a curators' notice that it was not requested, or no changed item to send.
 */
function trackValidations(AutoCurationService, notifications) {
  const proto = AutoCurationService.prototype
  const submissions = new Set()
  let started = 0
  let ended = 0
  const end = () => { ended += 1 }
  const submitted = proto.onChangesSubmittedInBackground
  proto.onChangesSubmittedInBackground = function (...args) {
    const running = submitted.apply(this, args)
    submissions.add(running)
    running.finally(() => submissions.delete(running)).catch(() => {})
    return running
  }
  const background = proto.validateInBackground
  proto.validateInBackground = function (...args) {
    started += 1
    return background.apply(this, args)
  }
  const changed = proto.findChangedItems
  proto.findChangedItems = function (...args) {
    return changed.apply(this, args).then(items => {
      if (!items.length) end()
      return items
    })
  }
  const notRequested = notifications.validationNotRequested
  notifications.validationNotRequested = (...args) => {
    end()
    return notRequested.apply(notifications, args)
  }
  return {
    end,
    settled: async () => {
      while (submissions.size) await Promise.allSettled([...submissions])
      for (let waited = 0; ended < started; waited += 1) {
        if (waited > 2000) fail(`${started - ended} validation requests did not end`)
        await new Promise(resolve => setTimeout(resolve, 5))
      }
    }
  }
}

function startFakes(req, unfaked, chain, forum) {
  const { parse } = req('graphql')
  const s3Objects = new Map()
  const warehoused = []
  const slacked = []
  const queued = []
  const newslettered = []
  const server = http.createServer((request, response) => {
    const chunks = []
    request.on('data', chunk => chunks.push(chunk))
    request.on('end', () => {
      const send = (status, body) => {
        response.writeHead(status, { 'content-type': 'application/json' })
        response.end(JSON.stringify(body))
      }
      if (request.method === 'POST' && request.url === '/peer/content/entities/active') return send(200, activeEntities(chain.entities, Buffer.concat(chunks)))
      if (request.method === 'GET' && /^\/peer\/lambdas\/profiles\/0x[0-9a-f]{40}$/.test(request.url)) return send(200, [])
      if (request.url.startsWith('/forum/')) {
        const answer = forumAnswer(request.method, request.url.slice('/forum'.length), request.headers, Buffer.concat(chunks), forum)
        if (!answer) unfaked.push(`forum ${request.method} ${request.url}`)
        return send(answer?.status ?? 404, answer?.body ?? { errors: ['not faked'] })
      }
      if (request.url.startsWith('/sns')) {
        const answer = snsAnswer(request.method, request.url, Buffer.concat(chunks), queued)
        if (!answer) {
          unfaked.push(`sns ${request.method} ${request.url}`)
          return send(400, { error: 'not faked' })
        }
        server.validationEnded?.()
        response.writeHead(answer.status, { 'content-type': 'text/xml' })
        return response.end(answer.body)
      }
      if (request.url.startsWith('/newsletter/')) {
        const answer = newsletterAnswer(request.method, request.url.slice('/newsletter'.length), request.headers, Buffer.concat(chunks), newslettered)
        if (!answer) unfaked.push(`newsletter ${request.method} ${request.url}`)
        return send(answer?.status ?? 404, answer?.body ?? { errors: ['not faked'] })
      }
      if (request.url.startsWith('/slack/')) {
        const answer = slackAnswer(request.method, request.url.slice('/slack'.length), request.headers, Buffer.concat(chunks), slacked)
        if (!answer) unfaked.push(`slack ${request.method} ${request.url}`)
        else if (!answer.body.ok) unfaked.push(`slack refused: ${answer.body.error}`)
        return send(answer?.status ?? 404, answer?.body ?? { ok: false, error: 'unknown_method' })
      }
      const warehouse = warehouseAnswer(request.method, request.url, request.headers, Buffer.concat(chunks), warehoused)
      if (warehouse) return send(warehouse.status, warehouse.body)
      if (request.url.startsWith('/s3/conformance/')) {
        const answer = s3Answer(request.method, request.url, request.headers, Buffer.concat(chunks), s3Objects)
        if (!answer) {
          unfaked.push(`s3 ${request.method} ${request.url}`)
          return send(404, { error: 'not faked' })
        }
        response.writeHead(answer.status, answer.headers)
        return response.end(answer.body)
      }
      if (request.method === 'POST' && request.url === '/rpc') {
        const { id, method, params = [] } = JSON.parse(Buffer.concat(chunks).toString('utf8'))
        const deployed = address => chain.collections.some(c => c.id === String(address).toLowerCase())
        if (method === 'eth_call') {
          const answer = oracleAnswer(params, chain.oracle)
          if (!answer) unfaked.push(`rpc eth_call ${JSON.stringify(params[0])}`)
          return send(200, { jsonrpc: '2.0', id, ...(answer || { result: '0x' }) })
        }
        const results = { eth_chainId: '0x89', net_version: '137', eth_getCode: deployed(params[0]) ? '0x6080' : '0x' }
        if (!(method in results)) unfaked.push(`rpc ${method}`)
        return send(200, { jsonrpc: '2.0', id, result: results[method] ?? null })
      }
      if (request.method !== 'POST' || !request.url.startsWith('/graph/')) {
        unfaked.push(`${request.method} ${request.url}`)
        return send(404, { error: 'not faked' })
      }
      const { query, variables = {} } = JSON.parse(Buffer.concat(chunks).toString('utf8'))
      const operation = parse(query).definitions.find(d => d.kind === 'OperationDefinition')
      const subgraph = request.url === '/graph/collections' ? chain : request.url === '/graph/third-parties' ? { thirdParties: chain.registry, receipts: chain.receipts } : {}
      const data = {}
      for (const field of operation.selectionSet.selections) {
        if (!FAKED_GRAPH_FIELDS.has(field.name.value)) unfaked.push(`graph ${field.name.value}`)
        const rows = subgraph[field.name.value]
        try {
          data[(field.alias || field.name).value] = rows ? selected(field, rows, variables, unfaked) : []
        } catch (error) {
          return send(200, { errors: [{ message: error.message }] })
        }
      }
      send(200, { data })
    })
  })
  server.slacked = slacked
  server.newslettered = newslettered
  server.queued = queued
  server.forum = forum
  return new Promise(resolve => server.listen(0, '127.0.0.1', () => resolve(server)))
}

function upstreamEnvironment(fakeBase, url, admins) {
  return {
    CONNECTION_STRING: url,
    ANALYTICS_CONNECTION_STRING: url,
    DEFAULT_ETH_ADDRESS: '0x0000000000000000000000000000000000000000',
    API_VERSION: 'v1',
    CHAIN_NAME: 'Ethereum Mainnet',
    COLLECTIONS_GRAPH_URL: `${fakeBase}/graph/collections`,
    THIRD_PARTY_GRAPH_URL: `${fakeBase}/graph/third-parties`,
    PEER_URL: `${fakeBase}/peer`,
    RPC_URL: `${fakeBase}/rpc`,
    FORUM_URL: FORUM_ORIGIN,
    FORUM_API_KEY: FORUM_KEY,
    FORUM_API_USERNAME: FORUM_USERNAME,
    FORUM_CATEGORY,
    OPEN_SEA_URL: `${fakeBase}/opensea`,
    OPEN_SEA_API_KEY: 'example-opensea-key',
    AWS_SNS_ARN: SNS_TOPIC,
    WEARABLE_VALIDATOR_CALLBACK_SECRET: VALIDATOR_SECRET,
    WAREHOUSE_URL: `${fakeBase}/warehouse`,
    WAREHOUSE_TOKEN,
    AWS_STORAGE_URL: `${fakeBase}/s3`,
    AWS_SNS_ENDPOINT: `${fakeBase}/sns`,
    AWS_BUCKET_NAME: 'conformance',
    AWS_ACCESS_KEY: 'example-access-key',
    AWS_ACCESS_SECRET: 'example-access-secret',
    AWS_ACCESS_KEY_ID: 'example-access-key',
    AWS_SECRET_ACCESS_KEY: 'example-access-secret',
    AWS_REGION: 'us-east-1',
    BUILDER_SERVER_URL: 'https://builder-api.example.org',
    CREATORS_URL: 'https://creators.example.org',
    FF_AUTO_CURATION: 'true',
    FF_AUTO_CURATION_REVIEW_FLOW: 'false',
    FF_COLLECTION_API_ALT: 'false',
    SLACK_BOT_TOKEN: SLACK_TOKEN,
    SLACK_CURATION_CHANNEL: SLACK_CHANNEL,
    NEWSLETTER_SERVICE_URL: `${fakeBase}/newsletter/v2`,
    NEWSLETTER_PUBLICATION_ID: NEWSLETTER_PUBLICATION,
    NEWSLETTER_SERVICE_API_KEY: NEWSLETTER_KEY,
    CORS_ORIGIN: CAPTURE_CORS_ORIGIN,
    ADMIN_ADDRESSES: admins,
    NODE_ENV: 'production'
  }
}

/** Runs upstream's migrations in a child process that loads the offline guard first. */
function migrate(upstream, environment) {
  execFileSync(process.execPath, [
    '--require', GUARD,
    path.join(upstream, 'migrations/node-pg-migrate'),
    '--database-url-var', 'CONNECTION_STRING',
    '--migration-file-language', 'ts',
    '--migrations-dir', path.join(upstream, 'migrations'),
    '--ignore-pattern', '\\..*|.*migrate(.ts)?',
    'up'
  ], { cwd: upstream, env: { ...process.env, ...environment }, stdio: ['ignore', 'ignore', 'pipe'] })
}

async function seedDatabase(client, seed, signers) {
  for (const provider of seed.third_parties) {
    await client.query(
      'INSERT INTO virtual_third_parties (id, managers, raw_metadata) VALUES ($1, $2, $3)',
      [provider.id, provider.managers.map(role => signers[role].address), `tp:1:${provider.name}:`]
    )
  }
  for (const collection of seed.collections) {
    await client.query(
      'INSERT INTO collections (id, name, eth_address, salt, contract_address, third_party_id, urn_suffix, linked_contract_address, linked_contract_network, forum_link, forum_id, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)',
      [collection.id, collection.name, collection.third_party_id ? '' : signers[collection.owner].address, collection.salt, collection.contract_address, collection.third_party_id ?? null, collection.urn_suffix ?? null, collection.linked_contract_address ?? null, collection.linked_contract_network ?? null, collection.forum_link ?? null, collection.forum_id ?? null, collection.created_at, collection.updated_at]
    )
  }
  for (const item of seed.items) {
    await client.query(
      'INSERT INTO items (id, name, eth_address, collection_id, blockchain_item_id, type, data, metrics, thumbnail, contents, rarity, urn_suffix, created_at, updated_at, local_content_hash) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)',
      [item.id, item.name, signers[item.owner].address, item.collection_id, item.blockchain_item_id, 'wearable', JSON.stringify(item.data), JSON.stringify(item.metrics), item.thumbnail, JSON.stringify(item.contents || {}), item.rarity ?? null, item.urn_suffix ?? null, item.created_at, item.updated_at, item.local_content_hash ?? null]
    )
  }
  for (const curation of seed.collection_curations) {
    await client.query(
      'INSERT INTO collection_curations (id, collection_id, status, assignee, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6)',
      [curation.id, curation.collection_id, curation.status, curation.assignee ? signers[curation.assignee].address : null, curation.created_at, curation.updated_at]
    )
  }
  for (const curation of seed.item_curations) {
    await client.query(
      'INSERT INTO item_curations (id, item_id, status, content_hash, is_mapping_complete, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7)',
      [curation.id, curation.item_id, curation.status, curation.content_hash, curation.is_mapping_complete, curation.created_at, curation.updated_at]
    )
  }
}

function scrub(value, parts) {
  if (Array.isArray(value)) return value.reduce((count, child) => count + scrub(child, parts), 0)
  if (!value || typeof value !== 'object' || !(parts[0] in value)) return 0
  if (parts.length > 1) return scrub(value[parts[0]], parts.slice(1))
  value[parts[0]] = VOLATILE
  return 1
}

/** Fails when upstream's own answer or the request already holds a redaction or volatile marker. */
function refuseMarkers(name, value, at = '') {
  if (typeof value === 'string') {
    if (value.includes('<<REDACTED')) fail(`${name}: upstream recorded <<REDACTED at ${at}`)
    if (value.includes(VOLATILE)) fail(`${name}: upstream recorded ${VOLATILE} at ${at}`)
    return
  }
  if (!value || typeof value !== 'object') return
  for (const [key, child] of Object.entries(value)) {
    refuseMarkers(name, child, Array.isArray(value) ? `${at}[${key}]` : at ? `${at}.${key}` : key)
  }
}

function valuesAt(value, parts) {
  if (Array.isArray(value)) return value.flatMap(child => valuesAt(child, parts))
  if (!parts.length) return [value]
  if (!value || typeof value !== 'object' || !(parts[0] in value)) return []
  return valuesAt(value[parts[0]], parts.slice(1))
}

/** The groups of the case's volatile paths whose recorded values were equal before masking. */
function volatileEqualities(item, body) {
  const groups = new Map()
  for (const volatile of item.volatile || []) {
    const values = valuesAt(body, volatile.split('.'))
    if (!values.length) continue
    const key = JSON.stringify(values)
    groups.set(key, [...(groups.get(key) || []), volatile])
  }
  return [...groups.values()].filter(group => group.length > 1)
}

/** Masks the case's volatile paths in `body`; each must name a field in `allowed` and match a recorded value. */
function scrubVolatile(item, body, allowed) {
  for (const volatile of item.volatile || []) {
    if (!allowed.includes(volatile.split('.').at(-1))) fail(`${item.name}: volatile path ${volatile} is not one of the volatile_fields`)
    if (!scrub(body, volatile.split('.'))) fail(`${item.name}: volatile path ${volatile} matches nothing`)
  }
  return body
}

/**
 * The `sign` value a capture keeps for `item` (the case's defect plus the metadata it signed when
 * timestamped), or undefined without one; refuses a defect the replay would not reproduce.
 */
function recordedSign(item) {
  if (!item.sign) return undefined
  const sign = item.sign.untimestamped ? { ...item.sign, ...(item.metadata ? { metadata: item.metadata } : {}) } : { ...item.sign, metadata: item.metadata || '{}' }
  const problems = []
  const defect = sign.untimestamped || sign.offset || sign.path || sign.method || sign.sentMetadata
  if (!defect) problems.push(sign.offset === 0 ? 'offset 0 is no defect' : 'names no defect')
  if (sign.withHeaders && !sign.untimestamped) problems.push('withHeaders needs untimestamped')
  if (sign.untimestamped && ['path', 'offset', 'metadata', 'sentMetadata'].some(key => key in sign)) {
    problems.push('untimestamped takes no path, offset, metadata or sentMetadata')
  }
  if (problems.length) fail(problems.map(problem => `${item.name}: sign ${problem}`).join('; '))
  return sign
}

function fixtureJson(fixture) {
  return JSON.stringify(fixture, null, 2) + '\n'
}

/**
 * What `item` sends and what its capture records of it: a JSON `body`, or `multipart` parts
 * (`name`, `filename`, `type` and the synthetic `text` of each file) sent as form data.
 */
function requestBody(item) {
  if (item.multipart === undefined) {
    return { body: item.body === undefined ? undefined : JSON.stringify(item.body), recorded: { body: item.body === undefined ? null : item.body } }
  }
  if (item.body !== undefined) fail(`${item.name}: a case sends a body or multipart parts, not both`)
  const form = new FormData()
  for (const part of item.multipart) {
    const keys = Object.keys(part).sort().join(',')
    if (keys !== 'filename,name,text,type') fail(`${item.name}: a multipart part names name, filename, type and text`)
    form.append(part.name, new Blob([Buffer.from(part.text, 'utf8')], { type: part.type }), part.filename)
  }
  return { body: form, recorded: { body: null, multipart: item.multipart } }
}

/** `value` with the fake bucket's origin `bucket` written as upstream's own bucket URL. */
function bucketNamed(value, bucket) {
  return bucket ? value.split(bucket).join(BUCKET_URL) : value
}

/**
 * The body `item` sends: its recorded body, except that a case with `validation_of` sends the validation id of
 * that collection's newest automated review start, which `latestStart` reads.
 */
async function sentBody(item, body, latestStart) {
  if (!item.validation_of) return body
  return JSON.stringify({ ...item.body, validationId: await latestStart(item.validation_of) })
}

async function record(req, base, signers, volatileFields, item, bucket, latestStart) {
  const query = item.query || {}
  const search = new URLSearchParams(Object.entries(query).flatMap(([key, value]) => [value].flat().map(v => [key, v]))).toString()
  const sent = requestBody({ ...item, body: await withCheque(item, signers) })
  sent.body = await sentBody(item, sent.body, latestStart)
  const headers = item.signer
    ? await signedHeaders(req, signers[item.signer], item)
    : item.validator ? validatorHeaders(item.validator, sent.body) : { ...item.headers }
  Object.assign(headers, item.send)
  if (item.body !== undefined) headers['content-type'] = 'application/json'
  const recordedHeaders = item.validation_of
    ? { ...validatorHeaders(item.validator, JSON.stringify(item.body)), 'content-type': 'application/json' }
    : headers
  for (let n = 0; n < (item.repeat ?? 0); n++) {
    const again = item.signer ? { ...(await signedHeaders(req, signers[item.signer], item)), ...item.send } : headers
    await (await fetch(base + item.path + (search ? '?' + search : ''), { method: item.method, headers: again, body: sent.body, redirect: 'manual', signal: AbortSignal.timeout(5000) })).arrayBuffer()
  }
  const response = await fetch(base + item.path + (search ? '?' + search : ''), {
    method: item.method,
    headers,
    body: sent.body,
    redirect: 'manual',
    signal: AbortSignal.timeout(5000)
  })
  const bytes = Buffer.from(bucketNamed(Buffer.from(await response.arrayBuffer()).toString('latin1'), bucket), 'latin1')
  const kept = {}
  for (const name of [...KEPT_RESPONSE_HEADERS, ...(item.keep ?? [])]) {
    const value = response.headers.get(name)
    if (value !== null) kept[name] = bucketNamed(value, bucket)
  }
  const recorded = { status: response.status, headers: kept }
  if ((kept['content-type'] || '').startsWith('application/json')) recorded.body_json = JSON.parse(bytes.toString('utf8'))
  else recorded.body_bytes_b64 = bytes.toString('base64')
  refuseMarkers(item.name, { request: { headers: recordedHeaders, query, ...sent.recorded }, response: recorded })
  const equalities = volatileEqualities(item, recorded.body_json)
  scrubVolatile(item, recorded.body_json, volatileFields)
  return {
    description: `${item.description}. Recorded from ${REPOSITORY} at ${REVISION.slice(0, 7)} by tooling/builder-server-capture.cjs.`,
    request: {
      method: item.method,
      path: item.path,
      query,
      headers: Object.fromEntries(Object.entries(recordedHeaders).sort(([a], [b]) => a.localeCompare(b))),
      ...sent.recorded
    },
    response: recorded,
    captured_from: `${REPOSITORY}@${REVISION}`,
    captured_at: CLOCK,
    volatile_paths: item.volatile || [],
    ...(equalities.length ? { volatile_equal: equalities } : {}),
    ...(item.headers ? { auth: 'recorded' } : {}),
    ...(item.sign ? { sign: recordedSign(item) } : {}),
    ...(item.validator ? { validator_sign: item.validator } : {}),
    ...(item.review_flow ? { review_flow: true } : {}),
    ...(item.validation_of ? { validation_of: item.validation_of } : {}),
    ...(item.repeat ? { repeat: item.repeat } : {})
  }
}

async function closeServer(server) {
  if (server) await new Promise((resolve, reject) => server.close(error => (error ? reject(error) : resolve())))
}

async function runUpstream(capture, url, seed, signers, unfaked) {
  const { upstream, req, load } = capture
  const fakes = await startFakes(req, unfaked, chainRows(seed, signers, req), forumState(seed.collections))
  let listener, db, unrouteSlack
  try {
    const environment = upstreamEnvironment(`http://127.0.0.1:${fakes.address().port}`, url, signers.admin.address)
    migrate(upstream, environment)
    const { Client } = req('pg')
    const client = new Client({ connectionString: url })
    await client.connect()
    try { await seedDatabase(client, seed, signers) } finally { await client.end() }
    Object.assign(process.env, environment)
    freezeClock()
    process.stdout.write = () => true
    process.stderr.write = () => true
    unrouteSlack = routeSlack(fakes.address().port)
    const { app } = load('server.ts')
    const notifications = load('Curation/notifications').curationNotifications
    const noticesSettled = trackNotices(notifications)
    const validations = trackValidations(load('Curation/AutoCuration.service').AutoCurationService, notifications)
    fakes.validationEnded = validations.end
    db = load('database').db
    await db.connect(url)
    listener = await new Promise(resolve => {
      const server = app.getApp().listen(0, '127.0.0.1', () => resolve(server))
    })
    const base = `http://127.0.0.1:${listener.address().port}`
    const fixtures = []
    const bucket = `${environment.AWS_STORAGE_URL}/${environment.AWS_BUCKET_NAME}`
    const latestStart = async collection => {
      const reader = new Client({ connectionString: url })
      await reader.connect()
      try {
        const { rows } = await reader.query(
          "SELECT payload->>'validationId' AS id FROM collection_events WHERE collection_id = $1 AND type = 'review.automated_started' ORDER BY created_at DESC, id DESC LIMIT 1",
          [collection]
        )
        if (!rows.length) fail(`no automated review started on ${collection}`)
        return rows[0].id
      } finally {
        await reader.end()
      }
    }
    for (const item of CASES) {
      const sent = fakes.slacked.length
      const subscribed = fakes.newslettered.length
      const called = fakes.forum.calls.length
      const requested = fakes.queued.length
      process.env.FF_AUTO_CURATION_REVIEW_FLOW = item.review_flow ? 'true' : 'false'
      const fixture = await capture.record(base, item, bucket, latestStart)
      await validations.settled()
      await noticesSettled()
      process.env.FF_AUTO_CURATION_REVIEW_FLOW = 'false'
      const slack = slackRecord(fakes.slacked.slice(sent))
      const forum = forumRecord(fakes.forum.calls.slice(called))
      const queue = queueRecord(fakes.queued.slice(requested))
      const newsletter = newsletterRecord(fakes.newslettered.slice(subscribed))
      refuseMarkers(item.name, newsletter)
      refuseMarkers(item.name, slack)
      refuseMarkers(item.name, forum)
      refuseMarkers(item.name, queue)
      fixtures.push([`${item.name}.json`, fixtureJson({ ...fixture, ...newsletter, ...slack, ...forum, ...queue })])
    }
    return fixtures
  } finally {
    try {
      await closeServer(listener)
      if (db) await db.close()
    } finally {
      unrouteSlack?.()
      await closeServer(fakes)
    }
  }
}

async function main() {
  const [root, output] = process.argv.slice(2)
  const ledger = JSON.parse(fs.readFileSync(path.join(FIXTURES, 'parity.json'), 'utf8'))
  const planErrors = [...caseOrderErrors(CASES), ...ledgerCaptureErrors(ledger, CASES)]
  if (planErrors.length) fail(planErrors.join('; '))
  const { refused } = require(GUARD)
  const { upstream } = assertPinnedSource(root, 'builder')
  if (!output) fail('Usage: node tooling/builder-server-capture.cjs <upstream-checkout> <output-directory>')
  const adminUrl = requireDatabaseUrl(process.env.CATALYST_TEST_DATABASE_URL)
  try { databaseUrl(adminUrl, 'postgres') } catch { fail('CATALYST_TEST_DATABASE_URL is not a valid URL') }
  assertLockedInstall(upstream)
  const capture = sourceCapture(upstream, { kind: 'builder' })
  const seed = readSeed()
  const signers = wallets(capture.req, seed)
  const { volatile_fields: volatileFields } = ledger
  capture.record = (base, item, bucket, latestStart) => record(capture.req, base, signers, volatileFields, item, bucket, latestStart)
  const unfaked = []
  const { Client } = capture.req('pg')
  const admin = new Client({ connectionString: databaseUrl(adminUrl, 'postgres') })
  await admin.connect()
  const fixtures = await withScratchDatabase(admin, adminUrl, url => runUpstream(capture, url, seed, signers, unfaked))
  finish(output, fixtures, [...refused, ...unfaked])
}

function failed(error) {
  report(`builder capture failed: ${redact(error?.message ?? error, process.env.CATALYST_TEST_DATABASE_URL)}\n`)
  Promise.resolve()
    .then(() => activeDrop?.())
    .catch(() => {})
    .finally(() => process.exit(1))
}

/** Reports an uncaught error or rejection redacted, drops the scratch database and exits with 1. */
function installFailureHandlers() {
  process.on('uncaughtException', failed)
  process.on('unhandledRejection', failed)
}

if (require.main === module) {
  installFailureHandlers()
  main().then(() => process.exit(0), failed)
}

module.exports = { EDGE_CASES, CAPTURE_CORS_ORIGIN, NEWSLETTER_KEY, NEWSLETTER_PUBLICATION, NEWSLETTER_FAILING_EMAIL, newsletterAnswer, newsletterRecord, sentBody, REVIEW_CASES, snsAnswer, queueRecord, validatorHeaders, trackValidations, VALIDATOR_SECRET, SNS_TOPIC, activeEntities, receiptRows, chequeTypedData, withCheque, CASES, SLACK_TOKEN, SLACK_CHANNEL, slackAnswer, slackRoute, slackRecord, FORUM_KEY, FORUM_USERNAME, FORUM_CATEGORY, forumState, forumAnswer, forumRoute, forumRecord, readSeed, seedDatabase, FAKED_GRAPH_FIELDS, WAREHOUSE_TOKEN, warehouseAnswer, oracleAnswer, rarityRows, registryRows, bucketNamed, caseOrderErrors, ledgerCaptureErrors, recordedSign, requestBody, s3Answer, withScratchDatabase, finish, redact, scrubVolatile, refuseMarkers, volatileEqualities, migrate, installFailureHandlers, selected }
