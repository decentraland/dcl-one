import { useEffect, useState } from "react";

import type { HeroDownloads } from "@/data/lib/download/creator-hub.server";
import { detectMacChip, isTouchMac, preferChip } from "@/data/lib/download/mac-chip";
import { trackCreatorHubClick, useCreatorHubDownloadClick } from "../download/creator-hub-click";
import CreatorHubDownload from "../download/CreatorHubDownload";

import { copy } from "./content";
import styles from "./CreateLanding.module.css";

export const CREATOR_HUB_DOWNLOAD = "/download/creator-hub";

/** An iPad names itself a Mac and has no installer; elsewhere the build for this Mac's chip comes first. */
function forThisDevice(downloads: HeroDownloads): HeroDownloads | null {
  if (downloads.primary.osKey !== "macos") return downloads;
  return isTouchMac() ? null : preferChip(detectMacChip(), downloads.primary, downloads.others);
}

type Props = {
  /** The installers for the visitor's system; null where there is none to offer in place. */
  downloads: HeroDownloads | null;
  sid: string;
  /** The verified wallet, sent as the download Click's `userId`. */
  userId?: string | null;
};

/** The hero's desktop call to action: the Creator Hub installer for this system, and the other builds under it. */
export default function HeroDownload({ downloads, sid, userId }: Props) {
  const [offered, setOffered] = useState(downloads);
  useEffect(() => setOffered(downloads && forThisDevice(downloads)), [downloads]);
  const onDownload = useCreatorHubDownloadClick({ sid, userId }, "create-hero");

  if (!offered) {
    return (
      <a
        className={`${styles.cta} ${styles.ctaDesktop}`}
        href={CREATOR_HUB_DOWNLOAD}
        onClick={() => trackCreatorHubClick("create-hero", { sid, userId })}
      >
        <span className={styles.ctaLabel}>{copy.downloadCreatorHub}</span>
      </a>
    );
  }

  return (
    <CreatorHubDownload
      primary={offered.primary}
      others={offered.others}
      onDownload={onDownload}
      className={styles.desktopOnly}
    />
  );
}
