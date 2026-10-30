// from: shop@94584a5c:app/src/lib/campaign.ts;
//       marketplace@f3956826:webapp/src/modules/event/actions.ts (FETCH_EVENT_REQUEST), webapp/src/modules/vendor/decentraland/builder/api.ts (fetchAddressesByTag)
// Licence of the Marketplace parts: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd.
// Which collections belong to a seasonal event: builder TAGS on whole collections, resolved through
// the Builder's public `GET /v1/addresses?tag=…` (the Builder boundary, /builder-api). An empty result
// means the event selects nothing (never "no filter"); an unreachable Builder throws so an outage is
// not mistaken for an empty event.

import { catalystBase } from "../../../data/catalyst/client";

export const MAX_CAMPAIGN_CONTRACTS = 120;

export async function fetchCampaignContracts(tags: string[], signal?: AbortSignal): Promise<string[]> {
  const wanted = tags.map((tag) => tag.trim()).filter(Boolean);
  if (wanted.length === 0) return [];
  const query = wanted.map((tag) => `tag=${encodeURIComponent(tag)}`).join("&");
  const res = await fetch(`${catalystBase()}/builder-api/v1/addresses?${query}`, { signal });
  if (!res.ok) throw new Error(`fetchCampaignContracts ${res.status}`);
  const json = (await res.json()) as { data?: string[] } | string[];
  const raw = Array.isArray(json) ? json : (json.data ?? []);
  const seen = new Set<string>();
  for (const address of raw) if (typeof address === "string" && /^0x[0-9a-fA-F]{40}$/.test(address)) seen.add(address.toLowerCase());
  return [...seen].slice(0, MAX_CAMPAIGN_CONTRACTS);
}

export function isInCampaign(contracts: string[], contractAddress: string | undefined | null): boolean {
  if (!contractAddress) return false;
  return contracts.includes(contractAddress.toLowerCase());
}
