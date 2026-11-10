// Public third-party store destinations (source Sites config/publicLinks.json and
// modules/downloadConstants.ts, googlePlayUrl.js). They are fixed URLs, not configuration.

export const EPIC_STORE_URL =
  "https://store.epicgames.com/en-US/p/decentraland-b692fb";
export const APP_STORE_URL =
  "https://apps.apple.com/app/apple-store/id6478403840?pt=126284288&ct=Decentraland%20Home%20iOS&mt=8";
const GOOGLE_PLAY_URL =
  "https://play.google.com/store/apps/details?id=org.decentraland.godotexplorer";

/** The site's own Play Store tag (its "QR code" attribution) when no live campaign arrived. */
const GOOGLE_PLAY_DEFAULT_UTM: Record<string, string> = {
  utm_org: "dclrgl",
  utm_source: "fdn",
  utm_medium: "qr",
  utm_campaign: "dclpage",
  utm_content: "android",
};

/**
 * The Play Store link with the visitor's campaign over the default tag, mirrored into `referrer`:
 * the Install Referrer API reads only that param, so without it no install joins a campaign.
 */
export function googlePlayUrl(campaign: Record<string, string> = {}): string {
  const url = new URL(GOOGLE_PLAY_URL);
  for (const [key, value] of Object.entries({
    ...GOOGLE_PLAY_DEFAULT_UTM,
    ...campaign,
  }))
    url.searchParams.set(key, value);
  const referrer = new URLSearchParams();
  for (const [key, value] of url.searchParams)
    if (key.startsWith("utm_")) referrer.append(key, value);
  url.searchParams.set("referrer", referrer.toString());
  return url.toString();
}
