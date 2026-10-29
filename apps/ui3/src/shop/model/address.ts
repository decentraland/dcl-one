// Mirrors src/shop/types.rs `EvmAddress`: 0x + 40 hex digits, compared by bytes, printed lowercase.

export type EvmAddress = `0x${string}` & { readonly __evmAddress: true };

const ADDRESS = /^0x[0-9a-fA-F]{40}$/;

export function parseEvmAddress(text: unknown): EvmAddress | null {
  if (typeof text !== "string" || !ADDRESS.test(text)) return null;
  return text.toLowerCase() as EvmAddress;
}

export function evmAddress(text: string): EvmAddress {
  const parsed = parseEvmAddress(text);
  if (!parsed) throw new TypeError(`not an EVM address: ${text}`);
  return parsed;
}

export function sameAddress(a: string | null | undefined, b: string | null | undefined): boolean {
  return !!a && !!b && a.toLowerCase() === b.toLowerCase();
}

export function shortAddress(address: string): string {
  return address.length > 12 ? `${address.slice(0, 6)}…${address.slice(-4)}` : address;
}
