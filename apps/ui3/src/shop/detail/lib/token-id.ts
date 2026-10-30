// from: shop@94584a5c:app/src/lib/token-id.ts
// ERC721CollectionV2 token ids: tokenId = (itemId << 216) | issuedId. The item can always be recovered
// from a token, but a bare number is not self-describing (item 0's tokens collide with item ids),
// which is why items and tokens have separate routes.

const ISSUED_ID_BITS = 216n;
const ISSUED_ID_MASK = (1n << ISSUED_ID_BITS) - 1n;

export type DecodedTokenId = { itemId: string; issuedId: string };

export function decodeTokenId(tokenId: string | bigint): DecodedTokenId {
  const n = typeof tokenId === "bigint" ? tokenId : BigInt(tokenId.trim());
  if (n < 0n) throw new Error(`Invalid tokenId: ${String(tokenId)}`);
  return { itemId: (n >> ISSUED_ID_BITS).toString(), issuedId: (n & ISSUED_ID_MASK).toString() };
}

export function encodeTokenId(itemId: string | bigint, issuedId: string | bigint): string {
  const item = typeof itemId === "bigint" ? itemId : BigInt(String(itemId).trim());
  const issued = typeof issuedId === "bigint" ? issuedId : BigInt(String(issuedId).trim());
  if (item < 0n || issued < 0n) throw new Error("Invalid itemId/issuedId");
  return ((item << ISSUED_ID_BITS) | issued).toString();
}

/** The item a token is a copy of, or null for a malformed value. */
export function itemIdFromTokenId(tokenId: string | null | undefined): string | null {
  if (!tokenId) return null;
  try {
    return decodeTokenId(tokenId).itemId;
  } catch {
    return null;
  }
}
