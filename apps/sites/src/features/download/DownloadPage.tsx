import { lazy, Suspense, useEffect, useRef, useState } from "react";

import { useChromeAuth, Button } from "@dcl/one-ui";
import logo from "@dcl/one-ui/apps/sites/assets/dcl-logo.svg?url";
import googlePlayBadge from "@dcl/one-ui/apps/sites/assets/google_play_cta.svg?url";
import { VerifiedIcon } from "@dcl/one-ui/apps/sites/download/VerifiedIcon";
import SitesLayout from "../components/chrome/SitesLayout";

import { successHref, type Installer } from "@/data/lib/download/installers";

import { trackDownloadClick, type DownloadClick } from "./download-click";
import { googlePlayHref } from "./google-play";

import appleLogo from "./assets/apple-logo.svg?url";
import appStoreBadge from "./assets/download-on-the-app-store.svg?url";
import epicIcon from "./assets/epic_icon.svg?url";
import googlePlayIcon from "./assets/google_play_icon.svg?url";
import iosLogo from "./assets/ios-logo.svg?url";
import microsoftLogo from "./assets/microsoft-logo.svg?url";
import styles from "./DownloadPage.module.css";

const WearablePreview = lazy(() => import("@ui/wearable-preview/WearablePreview"));

// The Explorer's download page. Signed out it shows only the
// logo and Sign In; signed in, the site nav, the account's ready line and its avatar. The reference renders the
// desktop or the phone layout by useDesktopMediaQuery (991px); both render here and CSS shows one.

const TITLE = "Download Decentraland\nand Come Hang Out!";
const EPIC_URL = "https://store.epicgames.com/en-US/p/decentraland-b692fb";
const APP_STORE_URL = "https://apps.apple.com/app/apple-store/id6478403840?pt=126284288&ct=Decentraland%20Home%20iOS&mt=8";

const OS_LABEL: Record<Installer["os"], string> = { windows: "Windows", macos: "macOS" };
const OS_IMAGE: Record<Installer["os"], string> = { windows: microsoftLogo, macos: appleLogo };

export type DownloadPageProps = {
  primary: Installer | null;
  secondary: Installer;
  /** Android phones get the Google Play badge instead of the App Store's. */
  android: boolean;
  /** The Explorer's download total ("+438K"), or null when unknown: then no total line (DownloadOptions). */
  downloads: string | null;
  /** The visitor's anonymous id, for the download clicks. */
  sid: string;
  /** The verified wallet, sent as the download clicks' `userId`. */
  userId?: string | null;
  /** The Google Play listing tagged with this visitor's campaign (`googlePlayHref`); the site's own tag by default. */
  googlePlay?: string;
};

// The legacy /download page's attributes per control (decentraland/sites `DownloadOptions/DownloadOptions.tsx`,
// `Layout/DownloadLayout.tsx`). The main button's place is `Download`, every other control's `download-page`.
const CLICKS = {
  primary: { place: "Download", event: "Download", download_target: "desktop_installer" },
  epic: { place: "download-page", event: "Download", download_target: "epic" },
  secondary: { place: "download-page", event: "Download", download_target: "desktop_installer" },
  appStore: { place: "download-page", event: "Download", download_target: "app_store", os: "iOS" },
  googlePlay: { place: "download-page", event: "Download", download_target: "google_play", os: "Android" },
} satisfies Record<string, DownloadClick>;

type OptionsProps = Pick<DownloadPageProps, "primary" | "secondary" | "downloads" | "sid" | "userId"> & { googlePlay: string };

function DownloadOptions({ primary, secondary, downloads, sid, userId, googlePlay }: OptionsProps) {
  if (!primary?.url) return null;
  const send = (click: DownloadClick) => () => trackDownloadClick(click, { sid, userId });
  return (
    <div className={styles.actions}>
      <div className={styles.buttons}>
        <Button
          className={styles.download}
          // The link is /download_success, the page that starts this installer: a plain link, so a new tab works too.
          href={successHref(primary)}
          onClick={send(CLICKS.primary)}
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
          onClick={send(CLICKS.epic)}
          aria-label="Download on Epic Games"
          trailingIcon={<img src={epicIcon} alt="Epic Games" width={32} height={32} />}
        >
          DOWNLOAD ON
        </Button>
      </div>
      <div className={styles.alternatives}>
        {downloads && (
          <p className={styles.counts}>
            <VerifiedIcon size={24} />
            {`Total Downloads: ${downloads}`}
          </p>
        )}
        <div className={styles.alternativeButtons}>
          {secondary.url && (
            <a className={styles.alternative} href={successHref(secondary)} onClick={send(CLICKS.secondary)} aria-label={OS_LABEL[secondary.os]}>
              <img src={OS_IMAGE[secondary.os]} alt="" />
            </a>
          )}
          <a className={styles.alternative} href={APP_STORE_URL} target="_blank" rel="noopener noreferrer" onClick={send(CLICKS.appStore)} aria-label="iOS">
            <img src={iosLogo} alt="" />
          </a>
          <a className={styles.alternative} href={googlePlay} target="_blank" rel="noopener noreferrer" onClick={send(CLICKS.googlePlay)} aria-label="Google Play">
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
      <div ref={ref} className={styles.preview} role="img" aria-label="Decentraland Avatar Preview">
        {near && (
          <Suspense fallback={null}>
            <WearablePreview profile={account} controls={false} spin={false} pauseOffscreen />
          </Suspense>
        )}
      </div>
    </div>
  );
}

export default function DownloadPage({
  primary,
  secondary,
  android,
  downloads,
  sid,
  userId,
  googlePlay = googlePlayHref(),
}: DownloadPageProps) {
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
                <p className={styles.preTitleText}>{`${auth.name || "Your Account"} is Ready!`}</p>
              </div>
            )}
            {/* The page's one heading: CSS shows this title on wide viewports and the one below on narrow ones, never both. */}
            <h1 className={styles.title}>{TITLE}</h1>
            <DownloadOptions primary={primary} secondary={secondary} downloads={downloads} sid={sid} userId={userId} googlePlay={googlePlay} />
          </div>
        </div>
        <h1 className={styles.mobileTitle}>{TITLE}</h1>
        {signedIn && <AvatarPreview account={auth.account} />}
      </div>
      <div className={styles.share}>
        <a
          className={styles.storeButton}
          href={android ? googlePlay : APP_STORE_URL}
          target="_blank"
          rel="noopener noreferrer"
          onClick={() => trackDownloadClick(android ? CLICKS.googlePlay : CLICKS.appStore, { sid, userId })}
        >
          <img className={styles.storeBadge} src={android ? googlePlayBadge : appStoreBadge} alt={android ? "Get it on Google Play" : "Download on the App Store"} />
        </a>
      </div>
    </main>
  );
  return (
    <div className={styles.frame}>
      <SitesLayout active="play" overlayNav hideNav={!signedIn}>
        {page}
      </SitesLayout>
    </div>
  );
}
