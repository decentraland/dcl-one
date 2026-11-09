// from: shop@94584a5c:app/src/lib/names.ts, marketplace@f3956826:webapp/src/modules/ens/{sagas,utils}.ts
// Copyright 2020 Metaverse Holdings Ltd. (marketplace sources). Licensed under the Apache License, Version 2.0.
// A NAME itself, before any purchase: its price, what may be typed and registered, its registrar token, and whether
// the registrar still has it. The registration, by credits or by MANA, is names.ts.

import { keccak256, parseAbi, stringToHex } from "viem";

import type { ChainId } from "@shop/model/chain";
import { landNetwork } from "../land/contracts";
import { readContract, type ChainReader } from "../land/read";

/** 100 MANA: DCLControllerV2's `PRICE` and the useCredits `maxCreditedValue` of a NAME. */
export const NAME_PRICE_IN_WEI = "100000000000000000000";
export const NAME_PRICE_WEI = 100n * 10n ** 18n;

export const NAME_MIN_LENGTH = 2;
export const NAME_MAX_LENGTH = 15;
const NAME_ALLOWED = /^[a-zA-Z0-9]+$/;

export type NameInvalidReason = "empty" | "too-short" | "too-long" | "invalid-chars" | "has-spaces";
export type NameValidation = { ok: true } | { ok: false; reason: NameInvalidReason };

/**
 * Validates a typed NAME: 2-15 ASCII letters or digits (`src/shop/types.rs` NameLabel::parse). A
 * space is reported apart from other characters, as the marketplace's claim page did.
 */
export function validateName(raw: string): NameValidation {
  const name = raw.trim();
  if (name.length === 0) return { ok: false, reason: "empty" };
  if (/\s/.test(name)) return { ok: false, reason: "has-spaces" };
  if (!NAME_ALLOWED.test(name)) return { ok: false, reason: "invalid-chars" };
  if (name.length < NAME_MIN_LENGTH) return { ok: false, reason: "too-short" };
  if (name.length > NAME_MAX_LENGTH) return { ok: false, reason: "too-long" };
  return { ok: true };
}

export const NAME_INVALID_TEXT: Record<Exclude<NameInvalidReason, "empty">, string> = {
  "too-short": "NAME too short: NAMEs must be at least 2 characters long.",
  "too-long": "NAMEs can contain up to 15 characters.",
  "invalid-chars": "NAMEs can only contain alphanumeric characters.",
  "has-spaces": "NAMEs can't contain spaces.",
};

/** How long typing rests before a NAME is checked. */
export const NAME_CHECK_DEBOUNCE_MS = 400;

/** Keeps a typed value registrable: drops anything but letters and digits and caps the length (at `max`). */
export function sanitizeNameInput(raw: string, max: number = NAME_MAX_LENGTH): string {
  return raw.replace(/[^a-zA-Z0-9]/g, "").slice(0, max);
}

/** The registrar token of a NAME: `uint256(keccak256(lower(label)))`, decimal. */
export function nameTokenIdOf(name: string): string {
  return BigInt(keccak256(stringToHex(name.toLowerCase()))).toString();
}

export const registrarAbi = parseAbi([
  "function available(string _subdomain) view returns (bool)",
  "function ownerOf(uint256 tokenId) view returns (address)",
]);

export type NameAvailability = "available" | "taken";

/**
 * `DCLRegistrar.available(name)` on `chainId` (the Ethereum network the Shop registers on, the host's choice), read
 * through `reader`: the gate the registration itself enforces. Advisory at search time; the server and the chain
 * decide at purchase.
 */
export async function checkNameAvailability(
  name: string,
  opts: { signal?: AbortSignal; reader: ChainReader; chainId: ChainId },
): Promise<NameAvailability> {
  const available = await readContract<boolean>(opts.reader, landNetwork(opts.chainId).nameRegistrar, registrarAbi, "available", [name], opts.signal);
  if (opts.signal?.aborted) throw new DOMException("Aborted", "AbortError");
  return available ? "available" : "taken";
}
