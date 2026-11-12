import { lazy, Suspense, useEffect, useRef, useState, type MouseEvent } from "react";

import { Button } from "@dcl/one-ui/components";
import { asset } from "@ui/asset";
import logo from "@ui/atoms/assets/decentraland-logo.svg?url";
import { useChromeAuth } from "@ui/web/frames/chrome-auth";
import SitesChrome from "@ui/web/frames/SitesChrome";

import { successHref, type Installer } from "@/data/lib/download/installers";

import appleLogo from "./assets/apple-logo.svg?url";
import appStoreBadge from "./assets/download-on-the-app-store.svg?url";
import epicIcon from "./assets/epic_icon.svg?url";
import googlePlayIcon from "./assets/google_play_icon.svg?url";
import iosLogo from "./assets/ios-logo.svg?url";
import microsoftLogo from "./assets/microsoft-logo.svg?url";
import styles from "./DownloadPage.module.css";

const WearablePreview = lazy(() => import("@ui/wearable-preview/WearablePreview"));

// from: sites@17b3535a:src/pages/download.tsx, src/components/Layout/DownloadLayout.tsx and
// src/components/DownloadOptions/DownloadOptions.tsx: the Explorer's download page. Signed out it shows only the
// logo and Sign In; signed in, the site nav, the account's ready line and its avatar. The reference renders the
// desktop or the phone layout by useDesktopMediaQuery (991px); both render here and CSS shows one.

const TITLE = "Download Decentraland\nand Come Hang Out!";
const EPIC_URL = "https://store.epicgames.com/en-US/p/decentraland-b692fb";
const APP_STORE_URL = "https://apps.apple.com/app/apple-store/id6478403840?pt=126284288&ct=Decentraland%20Home%20iOS&mt=8";
// from: sites@17b3535a:src/modules/downloadConstants.ts DOWNLOAD_URLS.googlePlay with GOOGLE_PLAY_DEFAULT_UTM.
const GOOGLE_PLAY_URL =
  "https://play.google.com/store/apps/details?id=org.decentraland.godotexplorer&utm_org=dclrgl&utm_source=fdn&utm_medium=qr&utm_campaign=dclpage&utm_content=android&referrer=utm_org%3Ddclrgl%26utm_source%3Dfdn%26utm_medium%3Dqr%26utm_campaign%3Ddclpage%26utm_content%3Dandroid";

const OS_LABEL: Record<Installer["os"], string> = { windows: "Windows", macos: "macOS" };
const OS_IMAGE: Record<Installer["os"], string> = { windows: microsoftLogo, macos: appleLogo };

export type DownloadPageProps = {
  primary: Installer | null;
  secondary: Installer;
  /** Android phones get the Google Play badge instead of the App Store's. */
  android: boolean;
  /** The Explorer's download total ("+438K"), or null when unknown: then no total line (DownloadOptions). */
  downloads: string | null;
};

/** A button goes to /download_success, which starts that installer (DownloadOptions handleDownloadOptionClick). */
function toSuccess(installer: Installer) {
  return (e: MouseEvent<HTMLAnchorElement>) => {
    e.preventDefault();
    window.location.href = successHref(installer);
  };
}

function DownloadOptions({ primary, secondary, downloads }: Pick<DownloadPageProps, "primary" | "secondary" | "downloads">) {
  if (!primary?.url) return null;
  return (
    <div className={styles.actions}>
      <div className={styles.buttons}>
        <Button
          className={styles.download}
          href={primary.url}
          onClick={toSuccess(primary)}
          aria-label={`Download for ${OS_LABEL[primary.os]}`}
          trailingIcon={<img src={OS_IMAGE[primary.os]} alt={OS_LABEL[primary.os]} width={32} height={32} />}
        >
          DOWNLOAD FOR
        </Button>
        <Button
          variant="secondary"
          className={styles.epic}
          href={EPIC_URL}
          target="_blank"
          rel="noopener noreferrer"
          aria-label="Download on Epic Games"
          trailingIcon={<img src={epicIcon} alt="Epic Games" width={32} height={32} />}
        >
          DOWNLOAD ON
        </Button>
      </div>
      <div className={styles.alternatives}>
        {downloads && (
          <p className={styles.counts}>
            <img src={asset("assets/home/verified.svg")} alt="" width={24} height={24} />
            {`Total Downloads: ${downloads}`}
          </p>
        )}
        <div className={styles.alternativeButtons}>
          {secondary.url && (
            <a className={styles.alternative} href={secondary.url} aria-label={OS_LABEL[secondary.os]} onClick={toSuccess(secondary)}>
              <img src={OS_IMAGE[secondary.os]} alt="" />
            </a>
          )}
          <a className={styles.alternative} href={APP_STORE_URL} target="_blank" rel="noopener noreferrer" aria-label="iOS">
            <img src={iosLogo} alt="" />
          </a>
          <a className={styles.alternative} href={GOOGLE_PLAY_URL} target="_blank" rel="noopener noreferrer" aria-label="Google Play">
            <img src={googlePlayIcon} alt="" />
          </a>
        </div>
      </div>
    </div>
  );
}

/** The avatar loads once its box is near the viewport, as the reference's useInView({ rootMargin: '200px' }) does. */
function AvatarPreview({ account }: { account: string }) {
  const ref = useRef<HTMLDivElement | null>(null);
  const [near, setNear] = useState(false);
  useEffect(() => {
    const node = ref.current;
    if (!node || near) return;
    const observer = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && setNear(true), { rootMargin: "200px" });
    observer.observe(node);
    return () => observer.disconnect();
  }, [near]);
  return (
    <div className={styles.image}>
      <div className={styles.imageOverlay} />
      <div ref={ref} className={styles.preview} aria-label="Decentraland Avatar Preview">
        {near && (
          <Suspense fallback={null}>
            <WearablePreview profile={account} controls={false} spin={false} pauseOffscreen />
          </Suspense>
        )}
      </div>
    </div>
  );
}

export default function DownloadPage({ primary, secondary, android, downloads }: DownloadPageProps) {
  const auth = useChromeAuth();
  const signedIn = auth.signedIn && !!auth.account;
  const page = (
    <main className={`${styles.page} ${signedIn ? styles.signedIn : styles.signedOut}`}>
      <div className={styles.container}>
        {!signedIn && (
          <>
            <a className={styles.logo} href="/" aria-label="Decentraland">
              <img src={logo} alt="" width={48} height={48} />
            </a>
            <button type="button" className={styles.signIn} onClick={auth.onSignIn}>
              Sign In
            </button>
          </>
        )}
        <div className={styles.desktop}>
          <div className={styles.alreadyDownloaded}>
            <p className={styles.alreadyText}>
              Already downloaded?{" "}
              <a className={styles.jumpIn} href="/play/">Jump In</a>
            </p>
          </div>
          <div className={styles.options}>
            {signedIn && (
              <div className={styles.preTitle}>
                <svg className={styles.check} viewBox="0 0 24 24" aria-hidden="true" data-testid="CheckCircleIcon">
                  <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2m-2 15-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8z" />
                </svg>
                <h4 className={styles.preTitleText}>{`${auth.name || "Your Account"} is Ready!`}</h4>
              </div>
            )}
            <h2 className={styles.title}>{TITLE}</h2>
            <DownloadOptions primary={primary} secondary={secondary} downloads={downloads} />
          </div>
        </div>
        <h2 className={styles.mobileTitle}>{TITLE}</h2>
        {signedIn && <AvatarPreview account={auth.account} />}
      </div>
      <div className={styles.share}>
        <a className={styles.storeButton} href={android ? GOOGLE_PLAY_URL : APP_STORE_URL} target="_blank" rel="noopener noreferrer">
          <img className={styles.storeBadge} src={android ? asset("assets/home/google-play.svg") : appStoreBadge} alt={android ? "Get it on Google Play" : "Download on the App Store"} />
        </a>
      </div>
    </main>
  );
  return (
    <div className={styles.frame}>
      <SitesChrome active="play" overlayNav hideNav={!signedIn}>
        {page}
      </SitesChrome>
    </div>
  );
}
