import { useContext } from "react";

import { sitesFooterCopy, PressKit, SitesLocaleContext, type SitesLocaleHandle } from "@dcl/one-ui";
import SitesChrome from "@ui/web/frames/SitesChrome";

import { pressCopy } from "@/features/press/locale";

import { canonicalLink } from "../lib/canonical";
import type { Route } from "./+types/press";

const TITLE = "Press Kit | Decentraland";
const DESCRIPTION =
  "Decentraland brand assets, screenshots and media resources for press and partners.";
const SOCIAL_IMAGE =
  "https://marketing-files.decentraland.org/uploads/1778186218133_decentraland-background.webp";
const SOCIAL_LOGO =
  "https://marketing-files.decentraland.org/uploads/1778267186976_DCL_icon_fullcolor.png";
const PRESS_EMAIL = "press@decentraland.org";
const PRESS_KIT_URL = "https://dcl.gg/press";

export const handle = { sitesLocalized: true } satisfies SitesLocaleHandle;

export function meta({ matches }: Route.MetaArgs) {
  const [canonical] = canonicalLink(
    matches[0]?.loaderData?.publicPageBase,
    "/press",
  );
  return [
    { name: "description", content: DESCRIPTION },
    { property: "og:type", content: "website" },
    { property: "og:title", content: TITLE },
    { property: "og:description", content: DESCRIPTION },
    { property: "og:site_name", content: "Decentraland" },
    { property: "og:image", content: SOCIAL_IMAGE },
    { property: "og:logo", content: SOCIAL_LOGO },
    { name: "twitter:card", content: "summary_large_image" },
    { name: "twitter:creator", content: "@decentraland" },
    { name: "twitter:site", content: "@decentraland" },
    { name: "twitter:title", content: TITLE },
    { name: "twitter:description", content: DESCRIPTION },
    { name: "twitter:image", content: SOCIAL_IMAGE },
    ...(canonical
      ? [canonical, { property: "og:url", content: canonical.href }]
      : []),
  ];
}

export default function Press() {
  const locale = useContext(SitesLocaleContext)?.locale ?? "en";
  const copy = pressCopy[locale];

  return (
    <SitesChrome active="none" footerCopy={sitesFooterCopy[locale]}>
      <title>{locale === "en" ? TITLE : copy.title}</title>
      <PressKit
        title={copy.title}
        description={copy.description}
        contactEmail={PRESS_EMAIL}
        downloadLabel={copy.download}
        downloadHref={PRESS_KIT_URL}
      />
    </SitesChrome>
  );
}
