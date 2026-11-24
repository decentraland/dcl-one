import { useEffect, useRef, type MouseEvent } from "react";
import { redirect, useNavigate } from "react-router";
import SitesLayout from "@/features/components/chrome/SitesLayout";
import { LegalPageLayout } from "@dcl/one-ui";
import { LEGAL_DOCS, type LegalKey } from "@/data/legal";
import { LEGAL_SIDEBAR, legalPath, resolveLegalKey } from "@/data/legal/nav";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import {
  pageLoader,
  type PageDefaults,
} from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";
import type { StoryId } from "@/core/lib/telemetry/story-id";
import { absoluteUrl, publicBase, socialMeta } from "../lib/social-meta";
import { isStaticPagePath, staticPageMeta } from "../lib/static-page-meta";
import type { Route } from "./+types/$slug";

const STORY: StoryId = "misc/legal";
export const handle = {
  agentMarkdown: "legalDoc",
} satisfies AgentMarkdownHandle;
const FALLBACK: PageDefaults = { variant: "config_driven" };

export async function loader({ request, context, params }: Route.LoaderArgs) {
  const slug = params.slug ?? "";
  if (slug === "seasons") {
    throw redirect("/shop/credits");
  }
  // The exact /credits path is the Credits API; its terms are served at /credits-terms.
  const key = slug === "credits" ? null : resolveLegalKey(slug);
  if (!key) {
    throw new Response(null, { status: 404, statusText: "Not Found" });
  }

  const { sid, wrap } = await pageLoader(request, context, FALLBACK);

  // Only the requested document reaches the client, not every one.
  const payload = { doc: key, legal: LEGAL_DOCS[key], sid };
  return wrap(payload);
}

export function meta({ loaderData, matches }: Route.MetaArgs) {
  if (!loaderData) {
    return socialMeta({ title: "Legal | Decentraland", description: "Decentraland policies and legal documents." });
  }
  const { doc, legal } = loaderData;
  // An alias (/rewards, /content-policy, ...) serves the same document as its slug.
  const path = legalPath(doc);
  const base = publicBase(matches);
  if (isStaticPagePath(path)) return staticPageMeta(path, base);
  return socialMeta({
    title: legal.pageTitle,
    description: `Read Decentraland’s ${legal.title}, including the policies and conditions that apply to the community.`,
    url: absoluteUrl(base, path),
  });
}

export default function LegalSlugRoute({ loaderData }: Route.ComponentProps) {
  const { doc, legal, sid } = loaderData;
  const navigate = useNavigate();
  const viewed = useRef<LegalKey | null>(null);
  useEffect(() => {
    if (viewed.current === doc) return;
    viewed.current = doc;
    track("lp_legal_viewed", { doc }, { sid, story: STORY });
  }, [doc, sid]);
  const onSectionClick = (event: MouseEvent<HTMLDivElement>) => {
    const anchor = (event.target as HTMLElement).closest("a");
    const href = anchor?.getAttribute("href");
    if (href?.startsWith("#"))
      track(
        "lp_legal_section_clicked",
        { doc, section: href.slice(1) },
        { sid, story: STORY },
      );
  };
  return (
    <SitesLayout active="legal" overlayNav="mobile">
      <main onClickCapture={onSectionClick}>
        <LegalPageLayout
          title={legal.title}
          toc={legal.toc}
          intro={legal.intro}
          sections={legal.sections}
          cta={legal.cta}
          sidebar={LEGAL_SIDEBAR.map(({ key, label, icon }) => ({
            href: legalPath(key),
            label,
            icon,
            active: key === doc,
          }))}
          sidebarLabel="Legal documents"
          onNavigate={(href) => void navigate(href)}
        />
      </main>
    </SitesLayout>
  );
}
