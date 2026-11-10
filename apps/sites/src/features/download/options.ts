import type { DownloadArch, DownloadOs } from "./params";

// Which installer /download offers (download contract §4; source Sites
// components/DownloadOptions/DownloadOptions.tsx:156-241 and modules/downloadLinks.ts). Detection
// runs on the request's user agent so the server render already shows the right buttons.

export type DetectedPlatform =
  | { kind: "unknown" }
  | { kind: "mobile"; os: "android" | "ios" }
  | {
      kind: "desktop";
      os: DownloadOs | "other";
      arch: DownloadArch | "ia32" | "unknown";
    };

export type DownloadOption = { os: DownloadOs; arch: DownloadArch };

export type DownloadOffer = {
  primary: DownloadOption | null;
  secondary: DownloadOption | null;
  /** Mobile visitors only see one store badge. */
  store: "app_store" | "google_play" | null;
};

/** A coarse user agent read: OS family, and for Windows the CPU the UA string names. */
export function detectPlatform(
  userAgent: string | null | undefined,
): DetectedPlatform {
  const ua = userAgent ?? "";
  if (!ua.trim()) return { kind: "unknown" };
  if (/Android/i.test(ua)) return { kind: "mobile", os: "android" };
  if (/iPhone|iPad|iPod/i.test(ua)) return { kind: "mobile", os: "ios" };
  if (/Windows NT/i.test(ua)) {
    const arch = /ARM64|aarch64/i.test(ua)
      ? "arm64"
      : /Win64|x64|WOW64|amd64/i.test(ua)
        ? "amd64"
        : "ia32";
    return { kind: "desktop", os: "windows", arch };
  }
  if (/Macintosh|Mac OS X/i.test(ua))
    return { kind: "desktop", os: "macos", arch: "unknown" };
  if (/Linux|CrOS|X11/i.test(ua))
    return { kind: "desktop", os: "other", arch: "unknown" };
  return { kind: "unknown" };
}

const WINDOWS: DownloadOption = { os: "windows", arch: "amd64" };
const MACOS: DownloadOption = { os: "macos", arch: "arm64" };

/**
 * Primary: macOS always gets the arm64 (universal) build; Windows its own arch, amd64 for arm64
 * (emulated) and none for ia32; an unknown or unsupported desktop gets Windows. Secondary: Windows
 * for Mac visitors, macOS for everyone else. As source, there is no Intel Mac option.
 */
export function downloadOffer(platform: DetectedPlatform): DownloadOffer {
  if (platform.kind === "mobile")
    return {
      primary: null,
      secondary: null,
      store: platform.os === "android" ? "google_play" : "app_store",
    };
  if (platform.kind === "unknown")
    return { primary: WINDOWS, secondary: MACOS, store: null };
  if (platform.os === "macos")
    return { primary: MACOS, secondary: WINDOWS, store: null };
  if (platform.os === "windows" && platform.arch === "ia32")
    return { primary: null, secondary: MACOS, store: null };
  return { primary: WINDOWS, secondary: MACOS, store: null };
}

/**
 * The home hero and /download choose their layout by viewport width, as source does, so they render
 * both: installers for a wide screen and one store badge for a narrow one. A phone UA on a wide
 * screen gets the default installers; the badge is Google Play only for Android, as source.
 */
export function viewportOffers(platform: DetectedPlatform): {
  wide: DownloadOffer;
  narrowStore: "app_store" | "google_play";
} {
  return {
    wide: downloadOffer(
      platform.kind === "mobile" ? { kind: "unknown" } : platform,
    ),
    narrowStore:
      platform.kind === "mobile" && platform.os === "android"
        ? "google_play"
        : "app_store",
  };
}
