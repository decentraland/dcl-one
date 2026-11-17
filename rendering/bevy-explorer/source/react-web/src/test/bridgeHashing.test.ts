import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { expect, it } from 'vitest'

const { hashV0, hashV1 } = createRequire(resolve('bridge-scene/package.json'))('@dcl/hashing') as {
  hashV0: (data: Uint8Array | AsyncIterable<Uint8Array>) => Promise<string>
  hashV1: (data: Uint8Array | AsyncIterable<Uint8Array>) => Promise<string>
}

const pinnedSdkHashes = [
  {
    "size": 0,
    "v0": "QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n",
    "v1": "bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku"
  },
  {
    "size": 1,
    "v0": "QmVkqaunyKCw4NKmWXbsGhkr5CotM8vCBKWyxsh8FGZyfr",
    "v1": "bafkreidogqfzz75tpkmjzjke425xqcrmpcib2p5tg44hnbirumdbpl5adu"
  },
  {
    "size": 262144,
    "v0": "QmRgPwRd8PpGwUY6g1yqHvqDMKeR3o5obr1fiNj5Nu3XNM",
    "v1": "bafkreibruh455iawsviqslif5c7uurdcfdemh22mtnytyzvnzn75kpejxy"
  },
  {
    "size": 262145,
    "v0": "QmVUgDpQtVyPdvzgVPV86s8ya9NCJkF54eYVkuzhZ487SZ",
    "v1": "bafybeiexg2oqkfnj56l7fcmawswqbijt5shq4b5rg6a546uwpkqqzwjioi"
  },
  {
    "size": 1048576,
    "v0": "QmV1XQY5kfiCsupnCCD9gjJeWBpvw6f1DhHLmJNmiPGf6Q",
    "v1": "bafybeiedpcapwld4tkgtzwahfofgn4wex5ryysf4se6hwpmlrsh4ntnrau"
  }
]

it.each(pinnedSdkHashes)('preserves the pinned SDK content hashes for $size bytes', async ({ size, v0, v1 }) => {
  const bytes = Uint8Array.from({ length: size }, (_, i) => i % 251)
  async function* chunks(): AsyncGenerator<Uint8Array> {
    for (let at = 0; at < bytes.length; at += 16387) yield bytes.subarray(at, at + 16387)
  }
  expect(await hashV0(bytes)).toBe(v0)
  expect(await hashV1(bytes)).toBe(v1)
  expect(await hashV0(chunks())).toBe(v0)
  expect(await hashV1(chunks())).toBe(v1)
})
