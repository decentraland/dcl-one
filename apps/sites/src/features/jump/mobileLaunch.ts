import { STORES } from "@/features/download/actions";
import type { DetectedPlatform } from "@/features/download/options";
import { parsePosition } from "@/data/lib/catalyst/jump/jump";

/** Play Store id of the mobile explorer (its `package/unique_name`). */
const ANDROID_PACKAGE = "org.decentraland.godotexplorer";

/** A phone has to switch apps before the page reports hidden; under about a second a real launch looks failed. */
export const LAUNCH_TIMEOUT_MS = 1500;

/** What a realm may look like: an ENS World name or a catalyst name. Anything else is dropped, not forwarded. */
const REALM = /^[a-zA-Z0-9._-]{1,64}$/;

export type MobileLaunch = { href: string; storeUrl: string; android: boolean };

/**
 * The app's `decentraland://open` route teleports with the position and realm it carries; a bare
 * `decentraland://` has no route. Both values are re-validated because they come from the page URL.
 */
export function mobileDeepLink(target: { coordinates: [number, number]; realm?: string }): string {
  const params = new URLSearchParams();
  const position = parsePosition(target.coordinates.join(","));
  if (position.isValid && position.coordinates.some((n) => n !== 0)) params.set("position", position.coordinates.join(","));
  if (target.realm && target.realm !== "main" && REALM.test(target.realm)) params.set("realm", target.realm);
  const query = params.toString();
  return query ? `decentraland://open?${query}` : "decentraland://open";
}

/** Chrome for Android resolves `intent://` against the installed package and opens the store itself when it is missing. */
export function androidIntentUrl(deepLink: string, storeUrl: string): string {
  const target = deepLink.replace(/^decentraland:\/\//, "");
  return `intent://${target}#Intent;scheme=decentraland;package=${ANDROID_PACKAGE};S.browser_fallback_url=${encodeURIComponent(storeUrl)};end`;
}

/** On a phone Jump In opens the installed app; null on desktop, where it opens the web explorer. */
export function mobileLaunch(platform: DetectedPlatform, target: { coordinates: [number, number]; realm?: string }): MobileLaunch | null {
  if (platform.kind !== "mobile") return null;
  const deepLink = mobileDeepLink(target);
  if (platform.os === "android") {
    return { href: androidIntentUrl(deepLink, STORES.google_play), storeUrl: STORES.google_play, android: true };
  }
  return { href: deepLink, storeUrl: STORES.app_store, android: false };
}

/**
 * iOS has no store fallback for an unknown scheme: fire the deep link and, when the page never loses
 * focus within the timeout, send the visitor to the App Store. Call it from the click itself, or the
 * navigation is blocked.
 */
export function openOnIos(launch: MobileLaunch, timeoutMs = LAUNCH_TIMEOUT_MS): Promise<boolean> {
  let left = false;
  const onLeave = () => {
    left = true;
  };
  const onVisibility = () => {
    if (document.hidden) onLeave();
  };
  document.addEventListener("visibilitychange", onVisibility);
  window.addEventListener("pagehide", onLeave);
  window.addEventListener("blur", onLeave);
  const cleanup = () => {
    document.removeEventListener("visibilitychange", onVisibility);
    window.removeEventListener("pagehide", onLeave);
    window.removeEventListener("blur", onLeave);
  };
  try {
    window.location.assign(launch.href);
  } catch {
    cleanup();
    window.location.assign(launch.storeUrl);
    return Promise.resolve(false);
  }
  return new Promise((resolve) => {
    setTimeout(() => {
      cleanup();
      if (!left) window.location.assign(launch.storeUrl);
      resolve(left);
    }, timeoutMs);
  });
}

/** Jump In's link and click for this visitor: the web explorer on desktop, the app on a phone. */
export function jumpInFor(
  playHref: string | undefined,
  launch: MobileLaunch | null,
): { jumpInHref?: string; onJumpIn?: (event: { preventDefault(): void }) => void } {
  if (!playHref) return {};
  if (!launch) return { jumpInHref: playHref };
  if (launch.android) return { jumpInHref: launch.href };
  return {
    jumpInHref: launch.href,
    onJumpIn: (event) => {
      event.preventDefault();
      void openOnIos(launch);
    },
  };
}
