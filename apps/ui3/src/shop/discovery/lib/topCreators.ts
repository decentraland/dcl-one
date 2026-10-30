// from: shop@2d3bf62:app/src/lib/topCreators.ts, app/src/lib/avatarColor.ts (getDisplayName, getAvatarBackgroundColor)
import { getAvatarBackgroundColor, getDisplayName } from "./avatarColor";

export type CreatorRank = {
  id: string;
  volumeWei?: string;
  sales: number;
  totalSales?: number;
  collections?: number;
  items?: number;
};

export type CreatorProfile = { name: string; hasClaimedName: boolean; face?: string };

export type TopCreator = {
  address: string;
  name: string;
  totalSales?: number;
  collections?: number;
  items?: number;
  face?: string;
};

export function selectTopCreators(ranked: CreatorRank[], profiles: Map<string, CreatorProfile>, limit: number): TopCreator[] {
  const taken = new Set<string>();
  const selected: TopCreator[] = [];
  for (const { id, totalSales, collections, items } of ranked) {
    if (selected.length >= limit) break;
    const profile = profiles.get(id.toLowerCase());
    const name = profile?.name?.trim();
    if (!profile?.hasClaimedName || !name) continue;
    const key = name.toLowerCase();
    if (taken.has(key)) continue;
    taken.add(key);
    selected.push({ address: id, name, totalSales, collections, items, face: profile.face });
  }
  return selected;
}

export function avatarBackgroundColor(name: string): string {
  return getAvatarBackgroundColor(getDisplayName(name, true, null));
}
