import type {
  DownloadActionsProps,
  DownloadAlternative,
  StoreBadgeProps,
} from "@dcl/one-ui/apps/sites/download/DownloadActions";
import { downloadActions, STORES } from "../download/actions";
import type { DetectedPlatform } from "../download/options";
import { homeCopy } from "./locale";

// The home page's download calls as decentraland.org renders them: "Download for" names the visitor's
// OS, or keeps a blank slot and leads to /download when the user agent names no OS the installers
// cover; Epic reads "Download on"; the stores are icon links beside the counter. The hero keeps each
// platform in a group of its own (the first one empty without a second installer); Come Hang Out
// puts the alternatives in one group at the counter's own width.

const STORE_LINKS: Record<"ios" | "android", DownloadAlternative> = {
  ios: {
    icon: "ios",
    label: "Download on the App Store",
    href: STORES.app_store,
    external: true,
  },
  android: {
    icon: "android",
    label: "Get it on Google Play",
    href: STORES.google_play,
    external: true,
  },
};

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
): HomeDownloadActions {
  const { actions, store } = downloadActions(platform, downloads);
  const covered =
    installers && platform.kind === "desktop" && platform.os !== "other";
  const shared: DownloadActionsProps = {
    ...actions,
    downloads: copy.total.replace("{downloads}", downloads),
    primary:
      covered && actions.primary
        ? { ...actions.primary, label: copy.forPlatform }
        : { label: copy.forPlatform, href: "/download" },
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
            href: STORES.google_play,
          },
  };
}
