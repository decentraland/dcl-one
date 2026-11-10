import "@dcl/one-ui/foundations/foundations.css";
import { useEffect, useRef, type MouseEvent } from "react";
import { redirect, useNavigate } from "react-router";
import SitesChrome from "@ui/web/frames/SitesChrome";
import { LegalPageLayout } from "@dcl/one-ui/apps/sites/legal/LegalPageLayout";
import type { LegalIconName } from "@dcl/one-ui/apps/sites/legal/LegalIcon";
import {
  LEGAL_DOCS,
  LEGAL_SIDEBAR,
  legalPath,
  resolveLegalKey,
  type LegalKey,
} from "@/data/legal";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import {
  pageLoader,
  type PageDefaults,
} from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";
import type { StoryId } from "@/core/lib/telemetry/story-id";
import type { Route } from "./+types/$slug";

const STORY: StoryId = "misc/legal";
export const handle = {
  agentMarkdown: "legalDoc",
} satisfies AgentMarkdownHandle;
const ICONS: Record<LegalKey, LegalIconName> = {
  brand: "favorite",
  content: "park",
  ethics: "balance",
  rewards: "cardGiftcard",
  credits: "token",
  security: "shield",
  privacy: "vpnKey",
  referral: "redeem",
  terms: "explore",
};
const FALLBACK: PageDefaults = { variant: "config_driven" };

export async function loader({ request, params }: Route.LoaderArgs) {
  if (params.slug === "seasons") throw redirect("/shop/credits");
  const key = resolveLegalKey(params.slug ?? "");
  if (!key) throw new Response(null, { status: 404, statusText: "Not Found" });
  const { sid, wrap } = await pageLoader(request, FALLBACK);
  return wrap({ doc: key, sid });
}

export function meta({ loaderData }: Route.MetaArgs) {
  const document = loaderData?.doc ? LEGAL_DOCS[loaderData.doc] : undefined;
  return [
    { title: document?.pageTitle ?? "Legal | Decentraland" },
    {
      name: "description",
      content: document
        ? `Read Decentraland’s ${document.title}, including the policies and conditions that apply to the community.`
        : "Decentraland policies and legal documents.",
    },
  ];
}

export default function LegalSlugRoute({ loaderData }: Route.ComponentProps) {
  const { doc, sid } = loaderData;
  const navigate = useNavigate();
  const viewed = useRef<LegalKey | null>(null);
  useEffect(() => {
    if (viewed.current === doc) return;
    viewed.current = doc;
    track("lp_legal_viewed", { doc }, { sid, story: STORY });
  }, [doc, sid]);
  const document = LEGAL_DOCS[doc];
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
    <SitesChrome active="legal" overlayNav>
      <main onClickCapture={onSectionClick}>
        <LegalPageLayout
          title={document.title}
          toc={document.toc}
          intro={document.intro}
          sections={document.sections}
          cta={document.cta}
          sidebar={LEGAL_SIDEBAR.map(({ key, label }) => ({
            href: legalPath(key),
            label,
            icon: ICONS[key],
            active: key === doc,
          }))}
          sidebarLabel="Legal documents"
          onNavigate={(href) => void navigate(href)}
        />
      </main>
    </SitesChrome>
  );
}
