import type { MouseEvent } from "react";
import SitesChrome from "../frames/SitesChrome";
import { asset } from "../../asset";
import logo from "../../atoms/assets/decentraland-logo.svg?url";
import styles from "./StCreatorHubDownload.module.css";

const L = {
  title: "Decentraland Creator Hub",
  alsoAvailable: "Also available on",
  downloadFor: (os: string) => `Download for ${os}`,
};

export const ICON = {
  Windows:
    "data:image/svg+xml;utf8," +
    encodeURIComponent(
      '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M0 3.449 9.75 2.1v9.451H0m10.949-9.602L24 0v11.4H10.949M0 12.6h9.75v9.451L0 20.699m10.949-8.099H24V24l-12.9-1.801"/></svg>',
    ),
  macOS:
    "data:image/svg+xml;utf8," +
    encodeURIComponent(
      '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M12.152 6.896c-.948 0-2.415-1.078-3.96-1.04-2.04.027-3.91 1.183-4.961 3.014-2.117 3.675-.546 9.103 1.519 12.09 1.013 1.454 2.208 3.09 3.792 3.039 1.52-.065 2.09-.987 3.935-.987 1.831 0 2.35.987 3.96.948 1.637-.026 2.676-1.48 3.676-2.948 1.156-1.688 1.636-3.325 1.662-3.415-.039-.013-3.182-1.221-3.22-4.857-.026-3.04 2.48-4.494 2.597-4.559-1.429-2.09-3.623-2.324-4.39-2.376-2-.156-3.675 1.09-4.61 1.09zM15.53 3.83c.843-1.012 1.4-2.427 1.245-3.83-1.207.052-2.662.805-3.532 1.818-.78.896-1.454 2.338-1.273 3.714 1.338.104 2.715-.688 3.559-1.701"/></svg>',
    ),
};

export type DownloadOption = {
  text: string;
  image: string;
  link: string;
  arch: string;
};

const PRIMARY: DownloadOption = {
  text: "macOS",
  image: ICON.macOS,
  link: "https://github.com/decentraland/creator-hub/releases/latest",
  arch: "arm64",
};
const SECONDARY: DownloadOption[] = [
  {
    text: "Windows",
    image: ICON.Windows,
    link: "https://github.com/decentraland/creator-hub/releases/latest",
    arch: "amd64",
  },
];

// from: sites@17b3535a:src/pages/download/CreatorHubDownload.tsx (decentraland-ui2 Logo, size "normal": 20px)
function DclLogo() {
  return <img className={styles.logo} src={logo} alt="Decentraland" width="20" height="20" />;
}

/** The reference's 752x500 PNG (sites: src/images/download/creator-hub), the same bytes. */
function BannerArt() {
  return <img className={styles.banner} src={asset("assets/creator-hub-banner.png")} width={752} height={500} alt="Decentraland Creator Hub" />;
}

const DownloadGlyph = ({ src, alt }: { src: string; alt: string }) => (
  <img className={styles.btnIcon} src={src} alt={alt} width="20" height="20" />
);

type StCreatorHubDownloadProps = {
  /** The installer for the visitor's system; none on phones, tablets and Linux, which then see no download actions. */
  primaryOption?: DownloadOption | null;
  secondaryOptions?: DownloadOption[];
  onDownload?: (option: DownloadOption, event: MouseEvent<HTMLAnchorElement>) => void;
};

export default function StCreatorHubDownload({
  primaryOption = PRIMARY,
  secondaryOptions = SECONDARY,
  onDownload = undefined,
}: StCreatorHubDownloadProps) {
  return (
    <SitesChrome active="none" overlayNav>
      <main className={styles.root}>
        <div className={styles.card}>
          <div className={styles.info}>
            <DclLogo />
            <h1 className={styles.title}>{L.title}</h1>

            {primaryOption?.link && (
              <div className={styles.actions}>
                <a
                  className={styles.cta}
                  href={primaryOption.link}
                  data-os={primaryOption.text}
                  onClick={onDownload && ((e) => onDownload(primaryOption, e))}
                >
                  <span className={styles.ctaLabel}>
                    {L.downloadFor(primaryOption.text)}
                  </span>
                  <DownloadGlyph src={primaryOption.image} alt="" />
                </a>

                {secondaryOptions.length > 0 && (
                  <div className={styles.also}>
                    <span className={styles.alsoText}>{L.alsoAvailable}</span>
                    {secondaryOptions.map((option) => (
                      <a
                        key={option.text}
                        className={styles.altIcon}
                        href={option.link}
                        data-os={option.text}
                        aria-label={L.downloadFor(option.text)}
                        onClick={onDownload && ((e) => onDownload(option, e))}
                      >
                        <img src={option.image} alt="" width="20" height="20" />
                      </a>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>

          <BannerArt />
        </div>
      </main>
    </SitesChrome>
  );
}
