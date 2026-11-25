import { concat, encodePacked, keccak256, type Hex } from 'viem'

type ContentHashTree = {
  merkleRoot: Hex
  proofs: Record<string, { index: number; proof: Hex[] }>
}

/** `keccak256(abi.encodePacked(index, contentHash))`: the leaf the third party registry checks a proof against. */
export const leafOf = (index: number, contentHash: string): Hex =>
  keccak256(encodePacked(['uint256', 'string'], [BigInt(index), contentHash]))

/**
 * A pair hashes in byte order, so a proof carries no left or right flags. Lowercase hex of one length sorts as its
 * bytes do.
 */
export const hashPair = (a: Hex, b: Hex): Hex => keccak256(a < b ? concat([a, b]) : concat([b, a]))

/**
 * The Merkle tree of a third party's content hashes, with the root and proofs `@dcl/content-hash-tree` gives: the
 * hashes are sorted, each leaf holds its position, and a node with no pair moves up as it is.
 */
export function generateTree(contentHashes: string[]): ContentHashTree {
  if (contentHashes.length === 0) throw new Error('empty tree')
  const sorted = [...contentHashes].sort()
  const leaves = sorted.map((contentHash, index) => leafOf(index, contentHash))
  const layers: Hex[][] = [[...leaves].sort()]
  for (let layer = layers[0]; layer.length > 1; layer = layers[layers.length - 1]) {
    const next: Hex[] = []
    for (let i = 0; i < layer.length; i += 2) {
      next.push(i + 1 < layer.length ? hashPair(layer[i], layer[i + 1]) : layer[i])
    }
    layers.push(next)
  }
  const positions = new Map(layers[0].map((leaf, position) => [leaf, position]))
  const proofs: ContentHashTree['proofs'] = {}
  sorted.forEach((contentHash, index) => {
    const proof: Hex[] = []
    let position = positions.get(leaves[index])!
    for (const layer of layers) {
      const pair = layer[position ^ 1]
      if (pair) proof.push(pair)
      position >>= 1
    }
    proofs[contentHash] = { index, proof }
  })
  return { merkleRoot: layers[layers.length - 1][0], proofs }
}
