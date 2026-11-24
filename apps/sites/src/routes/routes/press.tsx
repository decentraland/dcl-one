import { useContext } from "react";

import { sitesFooterCopy, PressKit, SitesLocaleContext, type SitesLocaleHandle } from "@dcl/one-ui";

import SitesLayout from "@/features/components/chrome/SitesLayout";
import { pressCopy } from "@/features/press/locale";

import { publicBase, SOCIAL_LOGO } from "../lib/social-meta";
import { STATIC_PAGE_CARDS, staticPageMeta } from "../lib/static-page-meta";
import type { Route } from "./+types/press";

const TITLE = `${STATIC_PAGE_CARDS["/press"].title} | Decentraland`;
const PRESS_EMAIL = "press@decentraland.org";
const PRESS_KIT_URL = "https://dcl.gg/press";

export const handle = { sitesLocalized: true } satisfies SitesLocaleHandle;

// The page renders its own localized <title>, so the head keeps only the card.
export function meta({ matches }: Route.MetaArgs) {
  return staticPageMeta("/press", publicBase(matches), {
    extra: [{ property: "og:logo", content: SOCIAL_LOGO }],
  }).filter((tag) => !("title" in tag));
}

export default function Press() {
  const locale = useContext(SitesLocaleContext)?.locale ?? "en";
  const copy = pressCopy[locale];

  return (
    <SitesLayout active="none" footerCopy={sitesFooterCopy[locale]}>
      <title>{locale === "en" ? TITLE : copy.title}</title>
      <PressKit
        title={copy.title}
        description={copy.description}
        contactEmail={PRESS_EMAIL}
        downloadLabel={copy.download}
        downloadHref={PRESS_KIT_URL}
      />
    </SitesLayout>
  );
}
