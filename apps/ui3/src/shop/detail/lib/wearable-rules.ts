// from: shop@94584a5c:app/src/lib/wearable-rules.ts
// What an equipped wearable hides, from its Catalyst entity (`hides`, `replaces`, and a skin covering
// the whole body), so a try-on on the shopper's avatar does not hide the very item being tried; and
// whether the creator blocked VRM export. Fail-soft: no answer leaves the avatar alone and shows no
// badge.

import { peerUrlFor } from "./urn";

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

type ActiveEntity = {
  pointers?: string[];
  metadata?: { data?: { category?: string; hides?: string[]; replaces?: string[]; blockVrmExport?: boolean } };
};

async function fetchActiveEntities(urns: string[], signal?: AbortSignal): Promise<ActiveEntity[]> {
  const res = await fetch(`${peerUrlFor(urns)}/content/entities/active`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ pointers: urns }),
    signal,
  });
  if (!res.ok) return [];
  return (await res.json()) as ActiveEntity[];
}

/** Whether VRM export is blocked for the wearable; null when it could not be read (no badge on a guess). */
export async function fetchVrmExportBlocked(urn: string, signal?: AbortSignal): Promise<boolean | null> {
  try {
    const data = (await fetchActiveEntities([urn], signal))[0]?.metadata?.data;
    return data ? !!data.blockVrmExport : null;
  } catch {
    return null;
  }
}

/** The rules for a list of equipped URNs in one request, in the order asked; unknown URNs are absent. */
export async function fetchWearableRules(urns: string[], signal?: AbortSignal): Promise<WearableRule[]> {
  if (urns.length === 0) return [];
  try {
    const entities = await fetchActiveEntities(urns, signal);
    const byUrn = new Map<string, WearableRule>();
    for (const entity of entities) {
      const data = entity.metadata?.data;
      if (!data?.category) continue;
      for (const pointer of entity.pointers ?? []) {
        byUrn.set(pointer.toLowerCase(), { urn: pointer, category: data.category, hides: data.hides ?? [], replaces: data.replaces ?? [] });
      }
    }
    return urns.map((urn) => byUrn.get(urn.toLowerCase())).filter((r): r is WearableRule => !!r);
  } catch {
    return [];
  }
}
