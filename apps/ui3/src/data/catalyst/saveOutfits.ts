import { privateKeyToAccount } from "viem/accounts";

import type { AuthLink } from "@web-core/auth/identity";
import { hexToColor3, isEthAddress, normalizeAddress } from "@web-core/catalyst/backpack";
import { catalystBase, type RequestOpts } from "@web-core/catalyst/client";
import { loadStoredIdentity } from "../auth/signedFetchLocal";

export type OutfitInput = {
  slot: number;
  bodyShape?: string;
  wearables?: string[];
  skinColor?: string;
  hairColor?: string;
  eyeColor?: string;
};

const MAX_BASE_OUTFIT_SLOTS = 5;

export function buildOutfitsMetadata(outfits: OutfitInput[]) {
  const seen = new Set<number>();
  const valid = (outfits ?? []).filter((o) => {
    const ok =
      Number.isInteger(o.slot) &&
      o.slot >= 0 &&
      o.slot < MAX_BASE_OUTFIT_SLOTS &&
      !!o.bodyShape &&
      !seen.has(o.slot);
    if (ok) seen.add(o.slot);
    return ok;
  });
  return {
    outfits: valid.map((o) => ({
      slot: o.slot,
      outfit: {
        bodyShape: o.bodyShape,
        eyes: { color: hexToColor3(o.eyeColor) },
        hair: { color: hexToColor3(o.hairColor) },
        skin: { color: hexToColor3(o.skinColor) },
        wearables: o.wearables ?? [],
        forceRender: [] as string[],
      },
    })),
    namesForExtraSlots: [] as string[],
  };
}

type SaveOutfitsResult =
  | { ok: true; entityId: string }
  | { ok: false; reason: string };

const RFC4648_BASE32_LOWER = "abcdefghijklmnopqrstuvwxyz234567";

function multibaseBase32Lower(data: Uint8Array): string {
  let bits = 0;
  let value = 0;
  let out = "";
  for (let i = 0; i < data.length; i++) {
    value = (value << 8) | data[i]!;
    bits += 8;
    while (bits >= 5) {
      out += RFC4648_BASE32_LOWER.charAt((value >>> (bits - 5)) & 31);
      bits -= 5;
    }
  }
  if (bits > 0) out += RFC4648_BASE32_LOWER.charAt((value << (5 - bits)) & 31);
  return `b${out}`;
}

function pushVarint(buf: number[], v: number): void {
  let value = v;
  for (;;) {
    const byte = value & 0x7f;
    value >>>= 7;
    if (value === 0) {
      buf.push(byte);
      break;
    }
    buf.push(byte | 0x80);
  }
}

async function computeEntityId(bytes: Uint8Array): Promise<string> {
  const digest = new Uint8Array(
    await crypto.subtle.digest("SHA-256", bytes as BufferSource),
  );
  const cid: number[] = [];
  pushVarint(cid, 1);
  pushVarint(cid, 0x55);
  cid.push(0x12, 0x20);
  for (const b of digest) cid.push(b);
  return multibaseBase32Lower(Uint8Array.from(cid));
}

export async function saveOutfits(
  address: string | null | undefined,
  outfits: OutfitInput[],
  opts: RequestOpts = {},
): Promise<SaveOutfitsResult> {
  const addr = normalizeAddress(address);
  if (!isEthAddress(addr)) {
    return { ok: false, reason: "connect a wallet to save outfits" };
  }

  const identity = loadStoredIdentity();
  if (!identity) {
    return { ok: false, reason: "sign in with a wallet to save outfits" };
  }

  const deployment = {
    version: "v3",
    type: "outfits",
    pointers: [`${addr}:outfits`],
    timestamp: Date.now(),
    content: [] as unknown[],
    metadata: buildOutfitsMetadata(outfits),
  };
  const entityBytes = new TextEncoder().encode(JSON.stringify(deployment));
  const entityId = await computeEntityId(entityBytes);

  const account = privateKeyToAccount(
    identity.ephemeralIdentity.privateKey as `0x${string}`,
  );
  const signature = await account.signMessage({ message: entityId });
  const authChain: AuthLink[] = [
    ...identity.authChain,
    { type: "ECDSA_SIGNED_ENTITY", payload: entityId, signature },
  ];

  const form = new FormData();
  form.set("entityId", entityId);
  authChain.forEach((link, i) => {
    form.set(`authChain[${i}][type]`, link.type);
    form.set(`authChain[${i}][payload]`, link.payload);
    form.set(`authChain[${i}][signature]`, link.signature ?? "");
  });
  form.set(
    entityId,
    new Blob([entityBytes], { type: "application/octet-stream" }),
    entityId,
  );

  const base = catalystBase(opts.base);
  let res: Response;
  try {
    res = await (opts.fetchImpl ?? fetch)(`${base}/content/entities`, {
      method: "POST",
      body: form,
      signal: opts.signal,
    });
  } catch (err) {
    if (opts.signal?.aborted) throw err;
    return { ok: false, reason: `outfits deploy request failed: ${String(err)}` };
  }

  if (!res.ok) {
    let detail = "";
    try {
      detail = await res.text();
    } catch {
    }
    return {
      ok: false,
      reason: `outfits deploy failed: ${res.status} ${res.statusText}${detail ? `: ${detail}` : ""}`,
    };
  }
  return { ok: true, entityId };
}
