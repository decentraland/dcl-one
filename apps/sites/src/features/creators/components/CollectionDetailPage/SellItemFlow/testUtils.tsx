import type { ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { TranslationProvider } from '@/data/lib/creators/intl'
import type { Session } from '@/data/lib/creators/auth'
import type { Collection } from '@/data/lib/creators/collections'
import { ItemType, type Item } from '@/data/lib/creators/items'

export const ADDRESS = '0x00000000000000000000000000000000000000aa'
export const FRIEND = '0x00000000000000000000000000000000000000bb'
export const CONTRACT = '0x00000000000000000000000000000000000000cc'

export const collection: Collection = {
  id: 'c1',
  name: 'Pirate Hats',
  owner: ADDRESS,
  urn: `urn:decentraland:amoy:collections-v2:${CONTRACT}`,
  contractAddress: CONTRACT,
  isPublished: true,
  isApproved: true,
  itemCount: 1,
  minters: [],
  managers: [],
  createdAt: 1,
  updatedAt: 1
}

export const item: Item = {
  id: 'i1',
  name: 'Pirate Hat',
  description: 'Yarr',
  thumbnail: 'thumbnail.png',
  owner: ADDRESS,
  collectionId: 'c1',
  rarity: 'legendary',
  totalSupply: 10,
  tokenId: '3',
  isPublished: true,
  isApproved: true,
  inCatalyst: true,
  type: ItemType.WEARABLE,
  data: {
    category: 'hat',
    representations: [
      { bodyShapes: ['urn:decentraland:off-chain:base-avatars:BaseFemale'], mainFile: 'hat.glb', contents: ['hat.glb'] }
    ]
  },
  contents: { 'hat.glb': 'Qmglb', 'thumbnail.png': 'Qmthumb', 'game.js': 'Qmjs' },
  createdAt: 1,
  updatedAt: 1
}

export function makeSession(kind: Session['kind'] = 'web3'): Session {
  return {
    address: ADDRESS,
    chainId: 11155111,
    kind,
    identity: {},
    provider: { request: () => Promise.resolve(null) }
  } as unknown as Session
}

export function Providers({ children }: { children: ReactNode }) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  return (
    <QueryClientProvider client={queryClient}>
      <TranslationProvider>{children}</TranslationProvider>
    </QueryClientProvider>
  )
}
