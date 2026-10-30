// from: shop@94584a5c:app/src/lib/iap.ts
// iOS in-app-purchase mode: the Shop inside the mobile app's web view must not sell credits (the app
// sells them). Same `?view=mobile-iap` the Marketplace uses. Read ONCE and sticky: the Shop's own
// navigations do not carry the parameter, and re-reading it would turn credit selling back on.

const VIEW_PARAM = "view";
const IAP_VIEW = "mobile-iap";

let cached: boolean | undefined;

export function isIapMode(): boolean {
  if (cached !== undefined) return cached;
  try {
    cached = new URLSearchParams(window.location.search).get(VIEW_PARAM) === IAP_VIEW;
  } catch {
    cached = false;
  }
  return cached;
}

/** Test seam: forget the memoised read. */
export function resetIapModeCache(): void {
  cached = undefined;
}
