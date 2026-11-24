import type { DownloadActionsProps, DownloadAlternative, StoreBadgeProps } from "@dcl/one-ui/apps/sites";
import { downloadActions, osName, STORES } from "../download/actions";
import { googlePlayHref } from "../download/google-play";
import type { DetectedPlatform } from "../download/options";
import { homeCopy } from "./locale";

// The home page's download calls, after decentraland/sites at dd905b2a98d3450d73c8f8c562881ad45ff3c83c
// (src/components/DownloadOptions/DownloadOptions.tsx), with one owner decision: "Download for"
// always shows an OS icon, macOS for a Mac visitor and Windows for everyone else, where source leaves
// the slot blank. The OS and installer come from `downloadActions`, so Linux and unidentified desktops
// start the Windows installer as /download does. The call leads to /download instead when the
// deployment configures no installer host, on phones, and on 32-bit Windows, which has no installer
// but keeps the Windows icon and the macOS link. Epic reads "Download on"; the stores are icon links
// beside the counter. The hero keeps each platform in a group of its own (the first one empty without
// a second installer); Come Hang Out puts the alternatives in one group at the counter's own width.

const storeLinks = (googlePlay: string): Record<"ios" | "android", DownloadAlternative> => ({
  ios: {
    icon: "ios",
    label: "Download on the App Store",
    href: STORES.app_store,
    external: true,
  },
  android: {
    icon: "android",
    label: "Get it on Google Play",
    href: googlePlay,
    external: true,
  },
});

export type HomeDownloadActions = {
  hero: DownloadActionsProps;
  hangOut: DownloadActionsProps;
  store: StoreBadgeProps;
  alternateStore?: StoreBadgeProps;
};

export function homeDownloadActions(
  platform: DetectedPlatform,
  downloads: string,
  /** False when the deployment configures no installer host: then the download call leads to /download. */
  installers: boolean,
  copy = homeCopy.en.download,
  /** The Google Play listing tagged with the visitor's campaign (`googlePlayHref`); the site's own tag by default. */
  googlePlay: string = googlePlayHref(),
): HomeDownloadActions {
  const { actions, store } = downloadActions(platform, downloads, googlePlay);
  const STORE_LINKS = storeLinks(googlePlay);
  // A desktop (or unidentified) visitor with an installer host gets the installers; phones do not.
  const covered = installers && platform.kind !== "mobile";
  // Only 32-bit Windows has no primary installer; it still shows the Windows icon.
  const { os, osName: name } = actions.primary ?? {
    os: "windows",
    osName: osName("windows"),
  };
  const shared: DownloadActionsProps = {
    ...actions,
    downloads: copy.total.replace("{downloads}", downloads),
    primary: {
      label: copy.forPlatform,
      os,
      osName: name,
      href: covered && actions.primary ? actions.primary.href : "/download",
    },
    epic: actions.epic && { ...actions.epic, label: copy.onStore },
  };
  const otherOs = covered ? (actions.alternativeGroups?.[0] ?? []) : [];
  return {
    hero: {
      ...shared,
      alternativeGroups: [otherOs, [STORE_LINKS.ios], [STORE_LINKS.android]],
    },
    hangOut: {
      ...shared,
      alternativeGroups: [[...otherOs, STORE_LINKS.ios, STORE_LINKS.android]],
      fit: true,
    },
    store,
    alternateStore:
      platform.kind === "mobile"
        ? undefined
        : {
            badge: "google_play",
            label: STORE_LINKS.android.label,
            href: googlePlay,
          },
  };
}
