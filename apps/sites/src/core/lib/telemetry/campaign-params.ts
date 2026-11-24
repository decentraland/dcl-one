/**
 * The partner-attribution `utm_*` parameters the legacy site copied into its download `Click` and funnel events,
 * normalized as it did (decentraland/sites `src/modules/campaignParams.ts`): read from the current URL at call
 * time, never persisted, so a visitor who browses away from the landing URL loses them.
 */
export const CAMPAIGN_PARAM_KEYS = ["utm_org", "utm_source", "utm_medium", "utm_campaign", "utm_content", "utm_term"] as const;

export type CampaignParamKey = (typeof CAMPAIGN_PARAM_KEYS)[number];
export type CampaignParams = Partial<Record<CampaignParamKey, string>>;

/** A partner-supplied value cannot flood the warehouse. */
const MAX_CAMPAIGN_VALUE_LENGTH = 256;

/** Lowercase, whitespace to `_`, only `[a-z0-9_-]`, no repeated or edge `_`. */
export function normalizeCampaignParamValue(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/\s+/g, "_")
    .replace(/[^a-z0-9_-]/g, "")
    .replace(/_+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, MAX_CAMPAIGN_VALUE_LENGTH);
}

/** The campaign parameters present in `search` (the current URL's by default); empty ones are left out. */
export function collectCampaignParams(search?: string): CampaignParams {
  const params = new URLSearchParams(search ?? (typeof window !== "undefined" ? window.location.search : ""));
  const collected: CampaignParams = {};
  for (const key of CAMPAIGN_PARAM_KEYS) {
    const value = params.get(key);
    const normalized = value ? normalizeCampaignParamValue(value) : "";
    if (normalized) collected[key] = normalized;
  }
  return collected;
}
