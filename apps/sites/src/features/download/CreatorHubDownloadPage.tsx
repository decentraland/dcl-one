import { useEffect, useRef, useState } from "react";

import { asset } from "@ui/asset";
import logo from "@ui/atoms/assets/decentraland-logo.svg?url";
import SitesChrome from "@ui/web/frames/SitesChrome";

import type { DownloadOption, OsKey } from "@/data/lib/download/creator-hub.server";
import { detectMacChip, preferChip } from "@/data/lib/download/mac-chip";
import { track } from "@/core/lib/telemetry/track";

import { CREATOR_HUB_DOWNLOAD_STORY, useCreatorHubDownloadClick } from "./creator-hub-click";
import OtherBuilds, { OsIcon } from "./OtherBuilds";
import styles from "./CreatorHubDownloadPage.module.css";

const copy = {
  title: "Decentraland Creator Hub",
  downloadFor: (os: string) => `Download for ${os}`,
};

type Props = {
  sid: string;
  version: string;
  /** The link named a system or a chip. */
  overridden: boolean;
  /** The link named a chip, which then stays first whatever this Mac's chip is. */
  archChosen: boolean;
  detectedOs: OsKey;
  detectedArch: string;
  /** None on phones, tablets and Linux, which then see the title and banner only. */
  primary: DownloadOption | null;
  secondary: DownloadOption[];
};

/** The Creator Hub download page: the installer for this system and the other builds under it. */
export default function CreatorHubDownloadPage({
  sid,
  version,
  overridden,
  archChosen,
  detectedOs,
  detectedArch,
  primary,
  secondary,
}: Props) {
  const [offered, setOffered] = useState({ primary, others: secondary });
  const onDownload = useCreatorHubDownloadClick(sid);
  const viewed = useRef<string | null>(null);

  useEffect(() => {
    const next = primary?.osKey === "macos" && !archChosen ? preferChip(detectMacChip(), primary, secondary) : { primary, others: secondary };
    setOffered(next);
    const os = next.primary?.osKey ?? detectedOs;
    const arch = next.primary?.arch ?? detectedArch;
    const key = `${os}|${arch}`;
    if (viewed.current === key) return;
    viewed.current = key;
    track("lp_creatorhub_download_viewed", { os, arch, version, overridden }, { sid, story: CREATOR_HUB_DOWNLOAD_STORY });
  }, [primary, secondary, archChosen, detectedOs, detectedArch, version, overridden, sid]);

  const main = offered.primary;
  return (
    <SitesChrome active="none" overlayNav>
      <main className={styles.root} data-os={main?.osKey} data-arch={main?.arch} data-version={version}>
        <div className={styles.card}>
          <div className={styles.info}>
            <img className={styles.logo} src={logo} alt="Decentraland" width="20" height="20" />
            <h1 className={styles.title}>{copy.title}</h1>

            {main && (
              <div className={styles.actions}>
                <a
                  className={styles.cta}
                  href={main.url}
                  data-os={main.osKey}
                  data-arch={main.arch}
                  onClick={(event) => onDownload(main, "primary", event)}
                >
                  <span className={styles.ctaLabel}>{copy.downloadFor(main.os)}</span>
                  <OsIcon os={main.osKey} className={styles.btnIcon} />
                </a>
                <OtherBuilds options={offered.others} onDownload={(option, event) => onDownload(option, "secondary", event)} />
              </div>
            )}
          </div>

          <img
            className={styles.banner}
            src={asset("assets/creator-hub-banner.png")}
            width={752}
            height={500}
            alt="Decentraland Creator Hub"
          />
        </div>
        {main && (
          <noscript>
            <a href={main.url}>Download Decentraland Creator Hub for {main.os}</a>
          </noscript>
        )}
      </main>
    </SitesChrome>
  );
}
