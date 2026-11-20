// from: shop@94584a5c:app/src/lib/wearable-rules.ts
// What an equipped wearable hides, from its Catalyst entity (`hides`, `replaces`, and a skin covering
// the whole body), so a try-on on the shopper's avatar does not hide the very item being tried; and
// whether the creator blocked VRM export. Fail-soft: no answer leaves the avatar alone and shows no
// badge.

import { shopPeers } from "../../api/host";
import { peerUrlFor } from "@shop/detail/lib/urn";
import { fetchEntities, type Entity } from "../../../wearable-preview/outfit";

export type WearableRule = { urn: string; category: string; hides: string[]; replaces: string[] };

const SKIN_COVERS = ["upper_body", "lower_body", "feet", "hands", "hands_wear", "head", "hair", "facial_hair", "eyebrows", "eyes", "mouth"];

export function hiddenBy(rule: WearableRule): Set<string> {
  const out = new Set<string>([...rule.hides, ...rule.replaces]);
  if (rule.category === "skin") for (const c of SKIN_COVERS) out.add(c);
  return out;
}

/** The equipped URNs to keep while trying on `tryOnCategories`: a same-slot item or one that would hide it goes. */
export function keepEquipped(equipped: WearableRule[], tryOnCategories: string[]): string[] {
  const wanted = new Set(tryOnCategories.filter(Boolean));
  if (wanted.size === 0) return equipped.map((r) => r.urn);
  return equipped
    .filter((rule) => {
      if (wanted.has(rule.category)) return false;
      const hidden = hiddenBy(rule);
      for (const category of wanted) if (hidden.has(category)) return false;
      return true;
    })
    .map((r) => r.urn);
}

const FACE_CATEGORIES = new Set(["eyes", "eyebrows", "mouth"]);

/** The equipped URNs an outfit preview keeps: the shopper's face and nothing else. */
export function faceOnly(equipped: WearableRule[]): string[] {
  return equipped.filter((rule) => FACE_CATEGORIES.has(rule.category)).map((r) => r.urn);
}

/** The Catalyst the item's entities are read from: the item's network's peer, else the host's base. */
export function entityBase(urns: string[]): string {
  return peerUrlFor(urns, shopPeers());
}

/**
 * The one Catalyst every URN's network reads from, or undefined when they span peers (a host that names a peer for one
 * network, and an outfit with items on two): no single peer then serves the whole outfit. A URN that carries no network
 * (the off-chain body shape, a collections-v1 wearable) does not decide, as in `peerUrlFor`: it is read from whichever
 * peer the others pick, as the rules read already does.
 */
export function sharedEntityBase(urns: string[]): string | undefined {
  const networked = urns.filter((urn) => /:(amoy|matic):/.test(urn));
  if (networked.length === 0) return entityBase(urns);
  const peers = new Set(networked.map((urn) => entityBase([urn])));
  return peers.size === 1 ? [...peers][0] : undefined;
}

/**
 * Entities by lowercased pointer, through the preview's entity cache: what is read here under `base`, a preview given
 * the same base does not read again.
 */
function readEntities(urns: string[], base: string = entityBase(urns)): Promise<Map<string, Entity>> {
  return fetchEntities(base, urns);
}

/** Whether VRM export is blocked for the wearable; null when it could not be read (no badge on a guess). */
export async function fetchVrmExportBlocked(urn: string): Promise<boolean | null> {
  try {
    const data = (await readEntities([urn])).get(urn.toLowerCase())?.metadata?.data;
    return data ? !!data.blockVrmExport : null;
  } catch {
    return null;
  }
}

/**
 * The rules for a list of equipped URNs, in the order asked; unknown URNs are absent. `alsoRead` (the body and the
 * item the preview mounts next) goes in the same request, so the preview finds those entities cached.
 */
export async function fetchWearableRules(urns: string[], alsoRead: string[] = [], base?: string): Promise<WearableRule[]> {
  if (urns.length === 0 && alsoRead.length === 0) return [];
  try {
    const entities = await readEntities([...urns, ...alsoRead], base);
    return urns.flatMap((urn) => {
      const data = entities.get(urn.toLowerCase())?.metadata?.data;
      return data?.category ? [{ urn, category: data.category, hides: data.hides ?? [], replaces: data.replaces ?? [] }] : [];
    });
  } catch {
    return [];
  }
}
