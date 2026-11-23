import { useQuery } from "@tanstack/react-query";

import { reelPlaceQuery, reelWearablesQuery } from "@/data/lib/reels/queries";

export function useReelPlace(position?: string, realm?: string) {
  return useQuery(reelPlaceQuery(position, realm));
}

export function useReelWearables(urns: readonly string[]) {
  return useQuery(reelWearablesQuery(urns));
}
