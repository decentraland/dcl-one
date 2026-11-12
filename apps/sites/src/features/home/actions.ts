import type {
  DownloadActionsProps,
  DownloadAlternative,
  StoreBadgeProps,
} from "@dcl/one-ui/apps/sites/download/DownloadActions";
import { downloadActions, STORES } from "../download/actions";
import type { DetectedPlatform } from "../download/options";

// The home page's download calls as decentraland.org renders them (sites@17b3535a,
// components/Home/Hero/Hero.tsx and ComeHangOut/ComeHangOut.tsx): "Download for" names the visitor's
// OS, or keeps a blank slot and leads to /download when the user agent names no OS the installers
// cover; Epic reads "Download on"; the stores are icon links beside the counter. The hero keeps each
// platform in a group of its own (the first one empty without a second installer); Come Hang Out
// puts both stores in one group at the counter's own width.

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
};

export function homeDownloadActions(
  platform: DetectedPlatform,
  downloads: string,
): HomeDownloadActions {
  const { actions, store } = downloadActions(platform, downloads);
  const covered = platform.kind === "desktop" && platform.os !== "other";
  const shared: DownloadActionsProps = {
    ...actions,
    primary: covered
      ? actions.primary
      : { label: "Download for", href: "/download" },
    epic: actions.epic && { ...actions.epic, label: "Download on" },
    alternatives: undefined,
  };
  const otherOs = covered ? (actions.alternatives ?? []) : [];
  return {
    hero: {
      ...shared,
      alternativeGroups: [otherOs, [STORE_LINKS.ios], [STORE_LINKS.android]],
    },
    hangOut: {
      ...shared,
      alternativeGroups: [[STORE_LINKS.ios, STORE_LINKS.android]],
      fit: true,
    },
    store,
  };
}
