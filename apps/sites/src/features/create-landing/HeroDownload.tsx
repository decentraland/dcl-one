import { useEffect, useState } from "react";

import type { HeroDownloads } from "@/data/lib/download/creator-hub.server";
import { detectMacChip, preferChip } from "@/data/lib/download/mac-chip";
import { useCreatorHubDownloadClick } from "../download/creator-hub-click";
import OtherBuilds, { downloadFor, OsIcon } from "../download/OtherBuilds";

import { copy } from "./content";
import styles from "./CreateLanding.module.css";

export const CREATOR_HUB_DOWNLOAD = "/download/creator-hub";

/** An iPad names itself a Mac and has no installer; elsewhere the build for this Mac's chip comes first. */
function forThisDevice(downloads: HeroDownloads): HeroDownloads | null {
  if (downloads.primary.osKey === "macos" && navigator.maxTouchPoints > 1) return null;
  return downloads.primary.osKey === "macos" ? preferChip(detectMacChip(), downloads.primary, downloads.others) : downloads;
}

type Props = {
  /** The installers for the visitor's system; null where there is none to offer in place. */
  downloads: HeroDownloads | null;
  sid: string;
};

/** The hero's desktop call to action: the Creator Hub installer for this system, and the other builds under it. */
export default function HeroDownload({ downloads, sid }: Props) {
  const [offered, setOffered] = useState(downloads);
  useEffect(() => setOffered(downloads && forThisDevice(downloads)), [downloads]);
  const onDownload = useCreatorHubDownloadClick(sid);

  if (!offered) {
    return (
      <a className={`${styles.cta} ${styles.ctaDesktop}`} href={CREATOR_HUB_DOWNLOAD}>
        <span className={styles.ctaLabel}>{copy.downloadCreatorHub}</span>
      </a>
    );
  }

  const { primary, others } = offered;
  return (
    <>
      <a
        className={`${styles.cta} ${styles.ctaDesktop}`}
        href={primary.url}
        title={downloadFor(primary)}
        data-os={primary.osKey}
        data-arch={primary.arch}
        onClick={(event) => onDownload(primary, "primary", event)}
      >
        <span className={styles.ctaLabel}>{copy.downloadCreatorHub}</span>
        <OsIcon os={primary.osKey} className={styles.osIcon} />
      </a>
      <OtherBuilds
        options={others}
        className={styles.desktopOnly}
        onDownload={(option, event) => onDownload(option, "secondary", event)}
      />
    </>
  );
}
