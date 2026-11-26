import { collectCampaignParams } from "@/core/lib/telemetry/campaign-params";

// The Google Play link the legacy site sent visitors to (decentraland/sites `src/modules/googlePlayUrl.js` and
// `src/modules/downloadConstants.ts`): the site's own "QR code" campaign tag, overlaid by the visitor's campaign.

export const GOOGLE_PLAY_BASE_URL = "https://play.google.com/store/apps/details?id=org.decentraland.godotexplorer";

export const GOOGLE_PLAY_DEFAULT_UTM = {
  utm_org: "dclrgl",
  utm_source: "fdn",
  utm_medium: "qr",
  utm_campaign: "dclpage",
  utm_content: "android",
} as const;

/**
 * Sets each `utm_*` on the base URL, then mirrors the final utm set into `referrer`: the bare parameters tag only
 * the store-page visit, while the Play Install Referrer API reads `referrer`, the one way an install joins a
 * campaign.
 */
export function buildGooglePlayUrl(baseUrl: string, utmParams: Record<string, string>): string {
  const url = new URL(baseUrl);
  for (const [key, value] of Object.entries(utmParams)) url.searchParams.set(key, value);
  const referrer = new URLSearchParams();
  for (const [key, value] of url.searchParams.entries()) {
    if (key.startsWith("utm_")) referrer.append(key, value);
  }
  // URLSearchParams percent-encodes the nested query (`utm_org%3D…%26…`), the form the API expects.
  url.searchParams.set("referrer", referrer.toString());
  return url.toString();
}

/** The Google Play link for a visitor who arrived with `search`: the defaults, overlaid by their normalized utm_*. */
export function googlePlayHref(search = ""): string {
  return buildGooglePlayUrl(GOOGLE_PLAY_BASE_URL, { ...GOOGLE_PLAY_DEFAULT_UTM, ...collectCampaignParams(search) });
}
