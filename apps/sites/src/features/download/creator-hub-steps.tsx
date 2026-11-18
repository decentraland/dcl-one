import type { ReactNode } from "react";

import macDownloads from "./assets/creator-hub/macos_downloads_folder.svg";
import macSetup from "./assets/creator-hub/mac_setup.svg";
import macAppIcon from "./assets/creator-hub/macos_app_icon.svg";
import windowsDownloads from "./assets/creator-hub/windows_downloads_folder.svg";
import windowsSetup from "./assets/creator-hub/windows_setup.svg";
import windowsAppIcon from "./assets/creator-hub/windows_app_icon.svg";

export type CreatorHubOs = "macos" | "windows";

type CreatorHubStep = { title: string; text: ReactNode; image: string };

const APP = <strong>Decentraland Creator Hub</strong>;

/** The three install steps of the Creator Hub download-success page, per system. */
export const CREATOR_HUB_STEPS: Record<CreatorHubOs, readonly CreatorHubStep[]> = {
  macos: [
    {
      title: "Open",
      text: <>Open the {APP} file from your Downloads Folder to begin the installation process.</>,
      image: macDownloads,
    },
    {
      title: "Install",
      text: (
        <>
          Drag and drop {APP} to the <strong>Applications folder</strong>. You might be asked to enter the admin password.
        </>
      ),
      image: macSetup,
    },
    {
      title: "Get Ready",
      text: (
        <>
          Locate {APP} in your <strong>Applications Folder</strong> and Start Building.
        </>
      ),
      image: macAppIcon,
    },
  ],
  windows: [
    {
      title: "Open",
      text: <>Open the {APP} file from your Downloads Folder to begin the installation process.</>,
      image: windowsDownloads,
    },
    {
      title: "Install",
      text: (
        <>
          Click <strong>Install</strong> and then <strong>Finish</strong>. {APP} will automatically start.
        </>
      ),
      image: windowsSetup,
    },
    { title: "Get ready", text: <>Start Building!</>, image: windowsAppIcon },
  ],
};

/** The latest Creator Hub release, offered again when the caller has no installer. */
export const CREATOR_HUB_RELEASES = "https://github.com/decentraland/creator-hub/releases/latest";
