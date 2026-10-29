// The root of an OpenZeppelin StandardMerkleTree (@openzeppelin/merkle-tree `StandardMerkleTree.of`,
// leaves sorted), the tree upstream coupons.ts signs over and marketplace-server rebuilds. Built on viem
// instead of adding the package: a leaf is keccak256(keccak256(abi.encode(types, value))), the leaves are
// sorted by hash and laid out from the end of an array heap, and each parent hashes its sorted pair.

import { concat, encodeAbiParameters, keccak256, type Hex } from "viem";

function compare(a: Hex, b: Hex): number {
  return a.toLowerCase() < b.toLowerCase() ? -1 : a.toLowerCase() > b.toLowerCase() ? 1 : 0;
}

export function standardLeafHash(types: readonly string[], value: readonly unknown[]): Hex {
  return keccak256(keccak256(encodeAbiParameters(types.map((type) => ({ type })), value)));
}

function hashPair(a: Hex, b: Hex): Hex {
  return keccak256(concat([a, b].sort(compare)));
}

/** `StandardMerkleTree.of(values, types).root`. Throws on an empty set, like the library. */
export function standardMerkleRoot(values: readonly (readonly unknown[])[], types: readonly string[]): Hex {
  if (values.length === 0) throw new Error("Expected non-zero number of leaves");
  const leaves = values.map((v) => standardLeafHash(types, v)).sort(compare);
  const tree: Hex[] = new Array<Hex>(2 * leaves.length - 1);
  leaves.forEach((leaf, i) => {
    tree[tree.length - 1 - i] = leaf;
  });
  for (let i = tree.length - 1 - leaves.length; i >= 0; i--) tree[i] = hashPair(tree[2 * i + 1]!, tree[2 * i + 2]!);
  return tree[0]!;
}
