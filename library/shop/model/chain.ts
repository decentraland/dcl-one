// Mirrors src/shop/types.rs `ChainId` (serde: a bare integer).

export type ChainId = number;

export const CHAIN = {
  ETHEREUM: 1,
  SEPOLIA: 11_155_111,
  POLYGON: 137,
  AMOY: 80_002,
} as const;

export function isEthereum(chainId: ChainId): boolean {
  return chainId === CHAIN.ETHEREUM || chainId === CHAIN.SEPOLIA;
}

/** The chains a CreditsManager lives on: credits only ever settle here. */
export function isPolygon(chainId: ChainId): boolean {
  return chainId === CHAIN.POLYGON || chainId === CHAIN.AMOY;
}

/** Ethereum purchases pay native gas from the buyer's wallet; Polygon ones go through the relay. */
export function nativeGasRequired(chainId: ChainId): boolean {
  return !isPolygon(chainId);
}

export function isChainId(value: unknown): value is ChainId {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0;
}

const LABELS: Record<number, string> = {
  [CHAIN.ETHEREUM]: "Ethereum",
  [CHAIN.SEPOLIA]: "Sepolia",
  [CHAIN.POLYGON]: "Polygon",
  [CHAIN.AMOY]: "Polygon Amoy",
};

export function chainLabel(chainId: ChainId): string {
  return LABELS[chainId] ?? `Chain ${chainId}`;
}

/** The token a wallet pays network fees with on `chainId`. */
export function gasTokenSymbol(chainId: ChainId): string {
  return isPolygon(chainId) ? "POL" : "ETH";
}
