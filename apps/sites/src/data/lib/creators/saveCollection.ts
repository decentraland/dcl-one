// Everything a collection save (create / rename) needs besides the HTTP call, ported from the
// legacy builder's save-collection saga: the placeholder URN, the ERC721CollectionV2.initialize
// calldata (builder-server derives the CREATE2 salt + contract address from it and rejects a save
// without it), and the symbol/items encodings that feed it.
import { encodeFunctionData, zeroAddress } from 'viem'
import { config } from '~/config'
import { type Collection } from './collections'
import { getItemMetadata, type Item } from './items'

// [rarity, price, beneficiary, metadata] — ERC721BaseCollectionV2.ItemParam.
export type InitializeItem = [string, string, string, string]

const INITIALIZE_ABI = [
  {
    inputs: [
      { internalType: 'string', name: '_name', type: 'string' },
      { internalType: 'string', name: '_symbol', type: 'string' },
      { internalType: 'string', name: '_baseURI', type: 'string' },
      { internalType: 'address', name: '_creator', type: 'address' },
      { internalType: 'bool', name: '_shouldComplete', type: 'bool' },
      { internalType: 'bool', name: '_isApproved', type: 'bool' },
      { internalType: 'contract IRarities', name: '_rarities', type: 'address' },
      {
        components: [
          { internalType: 'string', name: 'rarity', type: 'string' },
          { internalType: 'uint256', name: 'price', type: 'uint256' },
          { internalType: 'address', name: 'beneficiary', type: 'address' },
          { internalType: 'string', name: 'metadata', type: 'string' }
        ],
        internalType: 'struct ERC721BaseCollectionV2.ItemParam[]',
        name: '_items',
        type: 'tuple[]'
      }
    ],
    name: 'initialize',
    outputs: [],
    stateMutability: 'nonpayable',
    type: 'function'
  }
]

/** Legacy getCollectionSymbol: 'DCL-' + the name minus lowercase vowels and whitespace, uppercased. */
export function getCollectionSymbol(name: string): string {
  return 'DCL-' + name.replace(/a|e|i|o|u|\s/g, '').toUpperCase()
}

/** Placeholder URN for an unpublished collection; the real address is derived server-side. */
export function buildDefaultCatalystCollectionURN(): string {
  return `urn:decentraland:${config.get('MATIC_URN_PROTOCOL')}:collections-v2:${zeroAddress}`
}

/** A brand-new draft collection, exactly as the legacy CreateCollectionModal builds it. */
export function buildNewCollection(name: string, owner: string): Collection {
  const now = Date.now()
  return {
    id: crypto.randomUUID(),
    name,
    urn: buildDefaultCatalystCollectionURN(),
    owner,
    isPublished: false,
    isApproved: false,
    itemCount: 0,
    minters: [],
    managers: [],
    createdAt: now,
    updatedAt: now
  }
}

export function toInitializeItems(items: Item[]): InitializeItem[] {
  return [...items]
    .sort((a, b) => a.createdAt - b.createdAt)
    .map(item => [
      (item.rarity ?? '').toLowerCase(),
      item.price || '0',
      item.beneficiary ?? zeroAddress,
      getItemMetadata(item)
    ])
}

/** The ABI-encoded initialize calldata for a standard collection save. `from` is the signer. */
export function buildCollectionInitializeData(collection: Collection, items: Item[], from: string): string {
  return encodeFunctionData({
    abi: INITIALIZE_ABI,
    functionName: 'initialize',
    args: [
      collection.name,
      getCollectionSymbol(collection.name),
      config.get('ERC721_COLLECTION_BASE_URI'),
      from,
      true, // should complete
      false, // is approved
      config.get('RARITIES_WITH_ORACLE_ADDRESS'),
      toInitializeItems(items)
    ]
  })
}
