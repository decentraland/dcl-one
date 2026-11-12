import type { CSSProperties, ReactNode } from "react";
import SitesChrome from "../frames/SitesChrome";
import ManaMark from "../../atoms/ManaMark";
import { asset } from "../../asset";
import styles from "./StCreatorHubDownloadSuccess.module.css";

const AppleGlyph = () => (
  <svg viewBox="-52.01 0 560.035 560.035" fill="currentColor" aria-hidden="true">
    <path d="M380.844 297.529c.787 84.752 74.349 112.955 75.164 113.314-.622 1.988-11.754 40.191-38.756 79.652-23.343 34.117-47.568 68.107-85.731 68.811-37.499.691-49.557-22.236-92.429-22.236-42.859 0-56.256 21.533-91.753 22.928-36.837 1.395-64.889-36.891-88.424-70.883-48.093-69.53-84.846-196.475-35.496-282.165 24.516-42.554 68.328-69.501 115.882-70.192 36.173-.69 70.315 24.336 92.429 24.336 22.1 0 63.59-30.096 107.208-25.676 18.26.76 69.517 7.376 102.429 55.552-2.652 1.644-61.159 35.704-60.523 106.559M310.369 89.418C329.926 65.745 343.089 32.79 339.498 0 311.308 1.133 277.22 18.785 257 42.445c-18.121 20.952-33.991 54.487-29.709 86.628 31.421 2.431 63.52-15.967 83.078-39.655" />
  </svg>
);
const WindowsGlyph = () => (
  <svg viewBox="0 0 40 40" fill="currentColor" aria-hidden="true">
    <path d="M0 5.74875L16.25 3.50375V19.2525H0V5.74875ZM18.2488 3.24875L40 0V18.9963H18.2488V3.24875ZM0 21.0037H16.25V36.7525L0 34.4988V21.0037ZM18.2488 21.0037H40V40L18.5037 36.9988L18.2488 21.0037Z" />
  </svg>
);

const Diamond = () => <ManaMark size={34} />;

const hl = (t: ReactNode) => <span className={styles.hl}>{t}</span>;

type OSKey = "macos" | "windows";

type Step = {
  title: string;
  text: ReactNode;
  img: string;
};

const STEPS: Record<OSKey, Step[]> = {
  macos: [
    {
      title: "Open",
      text: (
        <>Open the {hl("Decentraland Creator Hub")} file from your Downloads Folder to begin the installation process.</>
      ),
      img: "assets/chs-macos_downloads_folder.svg",
    },
    {
      title: "Install",
      text: (
        <>Drag and drop {hl("Decentraland Creator Hub")} to the {hl("Applications folder")}. You might be asked to enter the admin password.</>
      ),
      img: "assets/chs-mac_setup.svg",
    },
    {
      title: "Get Ready",
      text: (
        <>Locate {hl("Decentraland Creator Hub")} in your {hl("Applications Folder")} and Start Building.</>
      ),
      img: "assets/chs-macos_app_icon.svg",
    },
  ],
  windows: [
    {
      title: "Open",
      text: (
        <>Open the {hl("Decentraland Creator Hub")} file from your Downloads Folder to begin the installation process.</>
      ),
      img: "assets/chs-windows_downloads_folder.svg",
    },
    {
      title: "Install",
      text: (
        <>Click {hl("Install")} and then {hl("Finish")}. {hl("Decentraland Creator Hub")} will automatically start.</>
      ),
      img: "assets/chs-windows_setup.svg",
    },
    {
      title: "Get ready",
      text: <>Start Building!</>,
      img: "assets/chs-windows_app_icon.svg",
    },
  ],
};

const RE_DOWNLOAD_HREF = "https://github.com/decentraland/creator-hub/releases/latest";

type StCreatorHubDownloadSuccessProps = {
  os?: OSKey;
  loading?: boolean;
  /** The installer the footer offers again; the latest release page when the caller has none. */
  retryUrl?: string;
};

// from: sites@17b3535a:src/pages/download/CreatorHubDownloadSuccess.tsx and pages/DownloadSuccess/DownloadSuccessLayout.tsx
export default function StCreatorHubDownloadSuccess({
  os = "macos",
  loading = false,
  retryUrl = RE_DOWNLOAD_HREF,
}: StCreatorHubDownloadSuccessProps) {
  const clientOS: OSKey = os === "windows" ? "windows" : "macos";
  const steps = STEPS[clientOS];

  return (
    <SitesChrome active="none" overlayNav>
      {loading && (
        <div className={styles.backdrop} role="status" aria-live="polite">
          <div className={styles.backBox}>
            <div className={styles.logo}>
              <Diamond />
              <span>Decentraland</span>
            </div>
            <div className={styles.backDetail}>
              <p className={styles.backText}>Downloading Decentraland...</p>
              <div className={styles.progress}><span /></div>
            </div>
          </div>
        </div>
      )}

      <main className={styles.root}>
        <header className={styles.header}>
          <span className={styles.osIcon}>
            {clientOS === "windows" ? <WindowsGlyph /> : <AppleGlyph />}
          </span>
          <h3 className={styles.title}>You&apos;re almost done!</h3>
          <p className={styles.subtitle}>
            Open the file you just downloaded to finish installing Decentraland and start creating
          </p>
        </header>

        <div className={styles.cards}>
          {steps.map((step, i) => (
            <article className={styles.card} key={i}>
              <div className={styles.cardContent}>
                <span className={styles.overline}>Step {i + 1}</span>
                <h4 className={styles.cardTitle}>{step.title}</h4>
                <p className={styles.cardText}>{step.text}</p>
              </div>
              <div className={styles.cardMedia} style={{ "--chds-media-img": `url("${asset(step.img)}")` } as CSSProperties} />
            </article>
          ))}
        </div>

        <p className={styles.footer}>
          Trouble with the download? Let&apos;s try that again:{" "}
          <a href={retryUrl} data-os={clientOS}>
            Download Decentraland Creator Hub
          </a>
        </p>
      </main>
    </SitesChrome>
  );
}
