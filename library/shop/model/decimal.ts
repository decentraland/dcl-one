// Mirrors src/shop/types.rs `parse_decimal` and the U256 bounds the Rust helpers check.

export const U256_MAX = (1n << 256n) - 1n;

/** A U256 as the wire carries it: 1-78 ASCII digits, below 2^256. */
export function parseDecimal(text: unknown): bigint | null {
  if (typeof text !== "string" || text.length === 0 || text.length > 78 || !/^[0-9]+$/.test(text)) return null;
  const value = BigInt(text);
  return value <= U256_MAX ? value : null;
}

/** The canonical decimal spelling (no leading zeros), as serde prints a U256. */
export function normalizeDecimal(text: unknown): string | null {
  const value = parseDecimal(text);
  return value === null ? null : value.toString();
}

export function inU256(value: bigint): boolean {
  return value >= 0n && value <= U256_MAX;
}

export function checkedU256(value: bigint): bigint | null {
  return inU256(value) ? value : null;
}

/** Ceiling division of non-negative bigints (`U256::div_ceil`). */
export function divCeil(a: bigint, b: bigint): bigint {
  return (a + b - 1n) / b;
}
