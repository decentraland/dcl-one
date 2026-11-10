// Mirrors src/shop/types.rs asset identity: an asset is chain + contract + TokenRef, keeping a
// collection item id (what a mint sells) apart from a minted token id (what a resale sells).

import { keccak256 } from "viem";

import { parseEvmAddress, type EvmAddress } from "./address";
import { isChainId, type ChainId } from "./chain";
import { normalizeDecimal } from "./decimal";

/** serde: `{"kind":"item","itemId":"7"}` | `{"kind":"token","tokenId":"42"}` (decimal strings). */
export type TokenRef = { kind: "item"; itemId: string } | { kind: "token"; tokenId: string };

/** serde: `{"chainId":137,"contract":"0x…","token":{…}}`. */
export type AssetId = { chainId: ChainId; contract: EvmAddress; token: TokenRef };

/** serde: `{"chainId":1,"contract":"0x…"}`. */
export type ContractRef = { chainId: ChainId; contract: EvmAddress };

export type AssetKind = "wearable" | "emote" | "name" | "parcel" | "estate" | "rental" | "other";
export const ASSET_KINDS: readonly AssetKind[] = ["wearable", "emote", "name", "parcel", "estate", "rental", "other"];

/** Derived by the server from the offer and the indexed records; the client never names it. */
export type SaleKind = "primaryMint" | "primaryNameRegistration" | "secondary";
export const SALE_KINDS: readonly SaleKind[] = ["primaryMint", "primaryNameRegistration", "secondary"];

export type ItemKind = "wearable" | "emote";

function decimalId(value: bigint | number | string): string {
  const text = typeof value === "string" ? value : value.toString();
  const normalized = normalizeDecimal(text);
  if (normalized === null) throw new TypeError(`not a U256 decimal id: ${text}`);
  return normalized;
}

export function itemAsset(chainId: ChainId, contract: EvmAddress, itemId: bigint | number | string): AssetId {
  return { chainId, contract, token: { kind: "item", itemId: decimalId(itemId) } };
}

export function tokenAsset(chainId: ChainId, contract: EvmAddress, tokenId: bigint | number | string): AssetId {
  return { chainId, contract, token: { kind: "token", tokenId: decimalId(tokenId) } };
}

export function tokenRefId(token: TokenRef): string {
  return token.kind === "item" ? token.itemId : token.tokenId;
}

/** The Rust `Display` form, `137:0x…:item:7`: a stable key for maps and cart lines. */
export function assetKey(asset: AssetId): string {
  return `${asset.chainId}:${asset.contract}:${asset.token.kind}:${tokenRefId(asset.token)}`;
}

export function parseAssetKey(key: string): AssetId | null {
  const [chain, contract, kind, id, ...rest] = key.split(":");
  if (rest.length > 0 || chain === undefined || id === undefined) return null;
  const chainId = Number(chain);
  const address = parseEvmAddress(contract);
  const normalized = normalizeDecimal(id);
  if (!isChainId(chainId) || !address || normalized === null) return null;
  if (kind === "item") return itemAsset(chainId, address, normalized);
  if (kind === "token") return tokenAsset(chainId, address, normalized);
  return null;
}

export function sameAsset(a: AssetId, b: AssetId): boolean {
  return assetKey(a) === assetKey(b);
}

/** Validates a wire AssetId, normalizing the address case and the decimal ids. */
export function parseAssetId(value: unknown): AssetId | null {
  if (!value || typeof value !== "object") return null;
  const v = value as { chainId?: unknown; contract?: unknown; token?: { kind?: unknown; itemId?: unknown; tokenId?: unknown } };
  const contract = parseEvmAddress(v.contract);
  if (!isChainId(v.chainId) || !contract || !v.token) return null;
  if (v.token.kind === "item") {
    const id = normalizeDecimal(v.token.itemId);
    return id === null ? null : itemAsset(v.chainId, contract, id);
  }
  if (v.token.kind === "token") {
    const id = normalizeDecimal(v.token.tokenId);
    return id === null ? null : tokenAsset(v.chainId, contract, id);
  }
  return null;
}

export type NameLabel = string & { readonly __nameLabel: true };

/** The registrar's rule (upstream `validateCreditLine`): 2-15 ASCII letters or digits, kept as typed. */
export function parseNameLabel(label: unknown): NameLabel | null {
  return typeof label === "string" && /^[A-Za-z0-9]{2,15}$/.test(label) ? (label as NameLabel) : null;
}

/** `DCLRegistrar.getTokenId`: `uint256(keccak256(lower(label)))`, as a decimal string. */
export function nameTokenId(label: NameLabel): string {
  const bytes = new TextEncoder().encode(label.toLowerCase());
  return BigInt(keccak256(bytes)).toString();
}

/** The registrar token a NAME is, or will be once registered. */
export function nameAsset(registrar: ContractRef, label: NameLabel): AssetId {
  return tokenAsset(registrar.chainId, registrar.contract, nameTokenId(label));
}
