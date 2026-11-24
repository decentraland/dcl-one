import type { DownloadActionsProps, StoreBadgeProps } from "@dcl/one-ui";
import { GOOGLE_PLAY_BASE_URL, googlePlayHref } from "./google-play";
import { downloadSuccessHref } from "./installers";
import { viewportOffers, type DetectedPlatform } from "./options";

export const STORES = {
  app_store:
    "https://apps.apple.com/app/apple-store/id6478403840?pt=126284288&ct=Decentraland%20Home%20iOS&mt=8",
  google_play: GOOGLE_PLAY_BASE_URL,
} as const;

export const osName = (os: "windows" | "macos") =>
  os === "windows" ? "Windows" : "macOS";

/** Desktop actions show installation guidance before requesting the pinned native artifact. */
export function downloadActions(
  platform: DetectedPlatform,
  downloads: string,
  /** The Google Play listing tagged with the visitor's campaign; the site's own tag by default. */
  googlePlay: string = googlePlayHref(),
): { actions: DownloadActionsProps; store: StoreBadgeProps } {
  const { wide, narrowStore } = viewportOffers(platform);
  return {
    actions: {
      primary: wide.primary
        ? {
            label: "Download for",
            os: wide.primary.os,
            osName: osName(wide.primary.os),
            href: downloadSuccessHref(wide.primary),
          }
        : undefined,
      epic: {
        label: "Get it on",
        iconLabel: "Epic Games",
        href: "https://store.epicgames.com/en-US/p/decentraland-b692fb",
      },
      downloads: `Total Downloads: ${downloads}`,
      alternativeGroups: wide.secondary
        ? [
            [
              {
                icon: wide.secondary.os,
                label: `Download for ${osName(wide.secondary.os)}`,
                href: downloadSuccessHref(wide.secondary),
              },
            ],
          ]
        : [],
    },
    store: {
      badge: narrowStore,
      label:
        narrowStore === "app_store"
          ? "Download on the App Store"
          : "Get it on Google Play",
      href: narrowStore === "google_play" ? googlePlay : STORES.app_store,
    },
  };
}
