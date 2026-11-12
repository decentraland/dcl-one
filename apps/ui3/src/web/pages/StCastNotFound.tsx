import { Button } from "@dcl/one-ui/components";

import { siteBase } from "../../data/site";
import SitesChrome from "../frames/SitesChrome";
import styles from "./StCastNotFound.module.css";

// Legacy class hooks: CastNotFound (apps/sites/src/features/social/cast-not-found) tracks a click on
// an anchor with one of these classes. They carry no styles.
const GO_HOME_HOOK = "stcastnotfound__btn";
const VIEW_DOCS_HOOK = "stcastnotfound__link";

// from: @mui/icons-material SportsEsports, Home, MenuBook (the icons sites@17b3535a's cast NotFound renders)
const SportsEsportsIcon = () => (
  <svg className={styles.glyph} viewBox="0 0 24 24" aria-hidden="true" focusable="false" data-testid="SportsEsportsIcon">
    <path d="m21.58 16.09-1.09-7.66C20.21 6.46 18.52 5 16.53 5H7.47C5.48 5 3.79 6.46 3.51 8.43l-1.09 7.66C2.2 17.63 3.39 19 4.94 19c.68 0 1.32-.27 1.8-.75L9 16h6l2.25 2.25c.48.48 1.13.75 1.8.75 1.56 0 2.75-1.37 2.53-2.91M11 11H9v2H8v-2H6v-1h2V8h1v2h2zm4-1c-.55 0-1-.45-1-1s.45-1 1-1 1 .45 1 1-.45 1-1 1m2 3c-.55 0-1-.45-1-1s.45-1 1-1 1 .45 1 1-.45 1-1 1" />
  </svg>
);

const HomeIcon = ({ className }: { className: string }) => (
  <svg className={className} viewBox="0 0 24 24" aria-hidden="true" focusable="false" data-testid="HomeIcon">
    <path d="M10 20v-6h4v6h5v-8h3L12 3 2 12h3v8z" />
  </svg>
);

const MenuBookIcon = ({ className }: { className: string }) => (
  <svg className={className} viewBox="0 0 24 24" aria-hidden="true" focusable="false" data-testid="MenuBookIcon">
    <path d="M21 5c-1.11-.35-2.33-.5-3.5-.5-1.95 0-4.05.4-5.5 1.5-1.45-1.1-3.55-1.5-5.5-1.5S2.45 4.9 1 6v14.65c0 .25.25.5.5.5.1 0 .15-.05.25-.05C3.1 20.45 5.05 20 6.5 20c1.95 0 4.05.4 5.5 1.5 1.35-.85 3.8-1.5 5.5-1.5 1.65 0 3.35.3 4.75 1.05.1.05.15.05.25.05.25 0 .5-.25.5-.5V6c-.6-.45-1.25-.75-2-1m0 13.5c-1.1-.35-2.3-.5-3.5-.5-1.7 0-4.15.65-5.5 1.5V8c1.35-.85 3.8-1.5 5.5-1.5 1.2 0 2.4.15 3.5.5z" />
    <path d="M17.5 10.5c.88 0 1.73.09 2.5.26V9.24c-.79-.15-1.64-.24-2.5-.24-1.7 0-3.24.29-4.5.83v1.66c1.13-.64 2.7-.99 4.5-.99M13 12.49v1.66c1.13-.64 2.7-.99 4.5-.99.88 0 1.73.09 2.5.26V11.9c-.79-.15-1.64-.24-2.5-.24-1.7 0-3.24.3-4.5.83m4.5 1.84c-1.7 0-3.24.29-4.5.83v1.66c1.13-.64 2.7-.99 4.5-.99.88 0 1.73.09 2.5.26v-1.52c-.79-.16-1.64-.24-2.5-.24" />
  </svg>
);

type StCastNotFoundProps = {
  title?: string;
  description?: string;
  goHomeLabel?: string;
  viewDocsLabel?: string;
  homeHref?: string;
  docsHref?: string;
};

export default function StCastNotFound({
  title = "404 - Page Not Found",
  description = "This Cast 2.0 stream doesn't exist. Stream links are generated on-demand by the Admin Smart Item in Decentraland scenes. If you followed a valid link and still see this error, the stream may have ended.",
  goHomeLabel = "Go Home",
  viewDocsLabel = "View Documentation",
  homeHref = siteBase(),
  docsHref = "https://docs.decentraland.org/creator/worlds/cast/",
}: StCastNotFoundProps) {
  return (
    <SitesChrome active="legal" overlayNav>
      <div className={styles.root}>
        <div className={styles.icon} aria-hidden="true">
          <SportsEsportsIcon />
        </div>

        <h1 className={styles.title}>{title}</h1>
        <p className={styles.description}>{description}</p>

        <div className={styles.actions}>
          <Button
            variant="primary"
            size="md"
            className={`${styles.goHome} ${GO_HOME_HOOK}`}
            href={homeHref}
            leadingIcon={<HomeIcon className={`${styles.glyph} ${styles.goHomeIcon}`} />}
          >
            {goHomeLabel}
          </Button>
          <Button
            variant="secondary"
            size="lg"
            className={`${styles.viewDocs} ${VIEW_DOCS_HOOK}`}
            href={docsHref}
            target="_blank"
            rel="noopener noreferrer"
            leadingIcon={<MenuBookIcon className={`${styles.glyph} ${styles.viewDocsGlyph}`} />}
          >
            {viewDocsLabel}
          </Button>
        </div>
      </div>
    </SitesChrome>
  );
}
