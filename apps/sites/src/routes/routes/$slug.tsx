import { useEffect, useRef } from "react";
import { redirect } from "react-router";

import LegalDocPageLayout from "@ui/web/frames/LegalDocPageLayout";
import { LEGAL_DOCS } from "@ui/data/legalPageConfig";
import "@ui/web/frames/legaldoc.css";

import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/$slug";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "misc/legal";

export const handle = { agentMarkdown: "legalDoc" } satisfies AgentMarkdownHandle;

const LEGAL_KEYS = [
  "terms",
  "privacy",
  "content",
  "ethics",
  "rewards",
  "referral",
  "security",
  "brand",
] as const;
type LegalKey = (typeof LEGAL_KEYS)[number];

const LEGAL_ALIAS: Record<string, LegalKey> = {
  "rewards-terms": "rewards",
  "referral-terms": "referral",
  "content-policy": "content",
};

function resolveLegalKey(slug: string): LegalKey | null {
  const key = LEGAL_ALIAS[slug] ?? slug;
  return (LEGAL_KEYS as readonly string[]).includes(key) ? (key as LegalKey) : null;
}

const FALLBACK: PageDefaults = {
  variant: "config_driven",
};

export async function loader({ request, params }: Route.LoaderArgs) {
  if ((params.slug ?? "") === "seasons") {
    throw redirect("/shop/credits");
  }
  const key = resolveLegalKey(params.slug ?? "");
  if (!key) {
    throw new Response(null, { status: 404, statusText: "Not Found" });
  }

  const { sid, wrap } = await pageLoader(
    request,
    
    FALLBACK,
  );

  const payload = { doc: key, sid };
  return wrap(payload);
}

export function meta({ loaderData }: Route.MetaArgs) {
  const doc = loaderData?.doc ? LEGAL_DOCS[loaderData.doc] : undefined;
  return [
    { title: doc ? `${doc.title} | Decentraland` : "Legal | Decentraland" },
    { name: "description", content: doc ? `Read Decentraland’s ${doc.title}, including the policies and conditions that apply to the community.` : "Decentraland policies and legal documents." },
  ];
}

type LoaderData = { doc: LegalKey; sid: string };

export default function LegalSlugRoute({ loaderData }: Route.ComponentProps) {
  const { doc, sid } = loaderData as LoaderData;

  const viewed = useRef(false);
  useEffect(() => {
    if (viewed.current) return;
    viewed.current = true;
    track("lp_legal_viewed", { doc }, { sid, story: STORY });
  }, [doc, sid]);

  const matched = LEGAL_DOCS[doc as LegalKey];

  function onSectionClick(e: React.MouseEvent<HTMLDivElement>) {
    const target = (e.target as HTMLElement).closest("a");
    if (!target) return;
    const href = target.getAttribute("href") ?? "";
    if (!href.startsWith("#")) return;
    track(
      "lp_legal_section_clicked",
      { doc, section: href.slice(1) },
      { sid, story: STORY },
    );
  }

  return (
    <div className="legal-doc-route" onClickCapture={onSectionClick}>
      <LegalDocPageLayout
        {...(({ doc: matched } satisfies Partial<
          React.ComponentProps<typeof LegalDocPageLayout>
        >) as React.ComponentProps<typeof LegalDocPageLayout>)}
      />
    </div>
  );
}
