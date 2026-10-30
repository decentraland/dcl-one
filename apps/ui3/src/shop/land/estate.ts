// from: marketplace@f3956826:webapp/src/modules/nft/estate/utils.ts (getFingerprint, computeEstateFingerprint),
// webapp/src/lib/estateUpgrade.ts
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// The Estate registry's composition fingerprint (renting an estate binds the rental to it) and the
// LR-XOR upgrade rule that makes some pre-upgrade estate listings non-executable.

import { encodeAbiParameters, keccak256, parseAbi, parseAbiParameters, type Hex } from "viem";

import { readContract, type ChainReader } from "./evm";

export const estateRegistryAbi = parseAbi([
  "function getFingerprintV2(uint256 estateId) view returns (bytes32)",
  "function getEstateSize(uint256 estateId) view returns (uint256)",
  "function estateLandIds(uint256 estateId, uint256 index) view returns (uint256)",
]);

/** The registry's `getFingerprintV2` for an estate, read now. */
export function readEstateFingerprint(reader: ChainReader, registry: string, estateId: string, signal?: AbortSignal): Promise<Hex> {
  return readContract<Hex>(reader, registry, estateRegistryAbi, "getFingerprintV2", [BigInt(estateId)], signal);
}

/** `getFingerprintV2`, computed off-chain from the land ids in the registry's storage order. */
export function computeEstateFingerprint(estateId: string, landIds: readonly string[]): Hex {
  return keccak256(encodeAbiParameters(parseAbiParameters("string, uint256, uint256[]"), ["estateId", BigInt(estateId), landIds.map((id) => BigInt(id))]));
}

/** A parcel carries no fingerprint: the zero word, as upstream's `randomBytes(32).map(() => 0)`. */
export const ZERO_FINGERPRINT: Hex = `0x${"00".repeat(32)}`;

/** Estates larger than this cannot honor a pre-upgrade (v1) fingerprint. */
export const ESTATE_LR_XOR_SAFE_MAX_SIZE = 18;
/** Unix seconds from which listings are signed against getFingerprintV2. */
export const ESTATE_V2_FINGERPRINT_LISTING_CUTOFF = 1779284232;

const toSeconds = (t: number) => (t > 1e12 ? Math.floor(t / 1000) : t);

/** A pre-upgrade listing on a large estate is no longer executable and must be re-created. */
export function isEstateListingAffectedByUpgrade(estateSize: number | null | undefined, listingCreatedAt: number | null | undefined): boolean {
  if (estateSize == null || estateSize <= ESTATE_LR_XOR_SAFE_MAX_SIZE) return false;
  if (listingCreatedAt == null) return false;
  return toSeconds(listingCreatedAt) < ESTATE_V2_FINGERPRINT_LISTING_CUTOFF;
}
