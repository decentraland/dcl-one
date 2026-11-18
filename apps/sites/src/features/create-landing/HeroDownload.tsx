import { useEffect, useRef, useState, type MouseEvent } from "react";

import type { DownloadOption, HeroDownloads } from "@/data/lib/download/creator-hub.server";
import { detectMacChip } from "@/data/lib/download/mac-chip";
import { track } from "@/core/lib/telemetry/track";
import type { StoryId } from "@/core/lib/telemetry/story-id";

import { copy } from "./content";
import styles from "./CreateLanding.module.css";

export const CREATOR_HUB_DOWNLOAD = "/download/creator-hub";

const STORY: StoryId = "landings/creator-hub-download";
// The installer starts downloading at once; the page that explains how to install it follows.
const SUCCESS_DELAY_MS = 3000;

const ICONS: Record<DownloadOption["osKey"], string> = {
  macos:
    "M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.81-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z",
  windows: "M2 3h9v9H2zm9 19H2v-9h9zM21 3v9h-9V3zm0 19h-9v-9h9z",
};

function OsIcon({ os }: { os: DownloadOption["osKey"] }) {
  return (
    <svg className={styles.osIcon} viewBox="0 0 24 24" width="20" height="20" fill="currentColor" aria-hidden="true">
      <path d={ICONS[os]} />
    </svg>
  );
}

/** A Mac build is named by its chip; Windows has one build. */
const buildName = (option: DownloadOption) => (option.osKey === "macos" ? `${option.os} (${option.archLabel})` : option.os);

/**
 * The server picks the Apple Silicon build for every Mac, whose User-Agent hides its chip. Where the browser
 * can tell the Mac is an Intel one (or the reverse), the build for it comes first.
 */
function forThisMac(downloads: HeroDownloads): HeroDownloads {
  if (downloads.primary.osKey !== "macos") return downloads;
  const chip = detectMacChip();
  const arch = chip === "intel" ? "amd64" : chip === "apple_silicon" ? "arm64" : null;
  if (!arch || arch === downloads.primary.arch) return downloads;
  const match = downloads.others.find((option) => option.osKey === "macos" && option.arch === arch);
  if (!match) return downloads;
  return { primary: match, others: downloads.others.map((option) => (option === match ? downloads.primary : option)) };
}

type Props = {
  /** The installers for the visitor's system; null where there is none to offer in place. */
  downloads: HeroDownloads | null;
  sid: string;
};

/** The hero's desktop call to action: the Creator Hub installer for this system, and the other builds under it. */
export default function HeroDownload({ downloads, sid }: Props) {
  const [offered, setOffered] = useState(downloads);
  useEffect(() => setOffered(downloads && forThisMac(downloads)), [downloads]);

  const redirect = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => {
    if (redirect.current !== null) clearTimeout(redirect.current);
  }, []);

  if (!offered) {
    return (
      <a className={`${styles.cta} ${styles.ctaDesktop}`} href={CREATOR_HUB_DOWNLOAD}>
        <span className={styles.ctaLabel}>{copy.downloadCreatorHub}</span>
      </a>
    );
  }

  const onDownload = (option: DownloadOption, kind: "primary" | "secondary", event: MouseEvent<HTMLAnchorElement>) => {
    track(
      "lp_creatorhub_download_clicked",
      { os: option.osKey, arch: option.arch, file_name: option.fileName, kind },
      { sid, story: STORY },
    );
    // A click that opens the file elsewhere (a new tab or window) leaves this page where it is.
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    if (redirect.current !== null) clearTimeout(redirect.current);
    redirect.current = setTimeout(() => {
      window.location.assign(`/download/creator-hub-success?${new URLSearchParams({ os: option.osKey, arch: option.arch })}`);
    }, SUCCESS_DELAY_MS);
  };

  const { primary, others } = offered;
  return (
    <>
      <a
        className={`${styles.cta} ${styles.ctaDesktop}`}
        href={primary.url}
        title={copy.downloadFor(buildName(primary))}
        data-os={primary.osKey}
        data-arch={primary.arch}
        onClick={(event) => onDownload(primary, "primary", event)}
      >
        <span className={styles.ctaLabel}>{copy.downloadCreatorHub}</span>
        <OsIcon os={primary.osKey} />
      </a>
      {others.length > 0 && (
        <div className={`${styles.otherBuilds} ${styles.desktopOnly}`}>
          {copy.alsoAvailableOn}
          {others.map((option) => (
            <a
              key={`${option.osKey}-${option.arch}`}
              className={styles.otherBuild}
              href={option.url}
              title={copy.downloadFor(buildName(option))}
              aria-label={copy.downloadFor(buildName(option))}
              data-os={option.osKey}
              data-arch={option.arch}
              onClick={(event) => onDownload(option, "secondary", event)}
            >
              <OsIcon os={option.osKey} />
              {option.osKey === "macos" && <span>{option.archLabel}</span>}
            </a>
          ))}
        </div>
      )}
    </>
  );
}
