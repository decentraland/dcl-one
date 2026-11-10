import type {
  DownloadActionsProps,
  StoreBadgeProps,
} from "@dcl/one-ui/apps/sites/download/DownloadActions";
import { viewportOffers, type DetectedPlatform } from "./options";

const INSTALLERS = {
  windows:
    "https://explorer-artifacts.decentraland.org/launcher-rust/Decentraland_installer.exe",
  macos:
    "https://explorer-artifacts.decentraland.org/launcher-rust/Decentraland_installer.dmg",
} as const;
const STORES = {
  app_store:
    "https://apps.apple.com/app/apple-store/id6478403840?pt=126284288&ct=Decentraland%20Home%20iOS&mt=8",
  google_play:
    "https://play.google.com/store/apps/details?id=org.decentraland.godotexplorer",
} as const;

/** Direct installers retain the pinned native artifact contract without a success-page dependency. */
export function homeActions(
  platform: DetectedPlatform,
  downloads: string,
): { actions: DownloadActionsProps; store: StoreBadgeProps } {
  const { wide, narrowStore } = viewportOffers(platform);
  const name = (os: "windows" | "macos") =>
    os === "windows" ? "Windows" : "macOS";
  return {
    actions: {
      primary: wide.primary
        ? {
            label: "Download for",
            os: wide.primary.os,
            osName: name(wide.primary.os),
            href: INSTALLERS[wide.primary.os],
          }
        : undefined,
      epic: {
        label: "Get it on",
        iconLabel: "Epic Games",
        href: "https://store.epicgames.com/en-US/p/decentraland-b692fb",
      },
      downloads: `Total Downloads: ${downloads}`,
      alternatives: wide.secondary
        ? [
            {
              icon: wide.secondary.os,
              label: `Download for ${name(wide.secondary.os)}`,
              href: INSTALLERS[wide.secondary.os],
            },
          ]
        : [],
    },
    store: {
      badge: narrowStore,
      label:
        narrowStore === "app_store"
          ? "Download on the App Store"
          : "Get it on Google Play",
      href: STORES[narrowStore],
    },
  };
}
