import { useEffect, useMemo, useRef, type MouseEvent } from "react";
import { redirect, useNavigate } from "react-router";

import { LegalPageLayout } from "@dcl/one-ui/apps/sites/legal/LegalPageLayout";
import type { LegalIconName } from "@dcl/one-ui/apps/sites/legal/LegalIcon";

import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import {
  LEGAL_DOCS,
  LEGAL_SIDEBAR,
  legalPath,
  resolveLegalKey,
  type LegalKey,
} from "@/data/legal";
import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";
import {
  loadLocaleMessages,
  type LocaleMessages,
} from "@/features/locale/catalogs";
import { LocaleProvider, useLocale } from "@/features/locale/LocaleProvider";
import {
  DEFAULT_LOCALE,
  resolveRequestLocale,
} from "@/features/locale/locales";

import SitesPageChrome from "../lib/sites-page-chrome";
import type { Route } from "./+types/$slug";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "misc/legal";

export const handle = {
  agentMarkdown: "legalDoc",
} satisfies AgentMarkdownHandle;

// Source's sidebar icons (Material): one per document.
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

  const [{ sid, wrap }, messages] = await Promise.all([
    pageLoader(request, FALLBACK),
    loadLocaleMessages(resolveRequestLocale(request)),
  ]);

  return wrap({ doc: key, sid, messages });
}

export function meta({ loaderData }: Route.MetaArgs) {
  const key = (loaderData as Partial<LoaderData> | undefined)?.doc;
  const doc = key ? LEGAL_DOCS[key] : undefined;
  return [
    { title: doc ? doc.pageTitle : "Legal | Decentraland" },
    {
      name: "description",
      content: doc
        ? `Read Decentraland’s ${doc.title}, including the policies and conditions that apply to the community.`
        : "Decentraland policies and legal documents.",
    },
  ];
}

type LoaderData = { doc: LegalKey; sid: string; messages: LocaleMessages };

export default function LegalSlugRoute({ loaderData }: Route.ComponentProps) {
  const { doc, sid, messages } = loaderData as LoaderData;

  // One view per document entered, also across client navigation between documents.
  const viewed = useRef<LegalKey | null>(null);
  useEffect(() => {
    if (viewed.current === doc) return;
    viewed.current = doc;
    track("lp_legal_viewed", { doc }, { sid, story: STORY });
  }, [doc, sid]);

  function onSectionClick(e: MouseEvent<HTMLDivElement>) {
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
    <LocaleProvider initial={messages}>
      <div onClickCapture={onSectionClick}>
        <LegalPage doc={doc} sid={sid} />
      </div>
    </LocaleProvider>
  );
}

function LegalPage({ doc, sid }: { doc: LegalKey; sid: string }) {
  const { locale } = useLocale();
  const navigate = useNavigate();
  const session = useMemo(() => ({ sid, story: STORY }), [sid]);
  const legal = LEGAL_DOCS[doc];
  return (
    <SitesPageChrome session={session} header={{ variant: "full" }}>
      <LegalPageLayout
        title={legal.title}
        toc={legal.toc}
        intro={legal.intro}
        sections={legal.sections}
        cta={legal.cta}
        sidebar={LEGAL_SIDEBAR.map(({ key, label }) => ({
          href: legalPath(key),
          label,
          icon: ICONS[key],
          active: key === doc,
        }))}
        sidebarLabel="Legal documents"
        // Legal text is English in every locale, as source.
        lang={locale === DEFAULT_LOCALE ? undefined : DEFAULT_LOCALE}
        onNavigate={(href) => void navigate(href)}
      />
    </SitesPageChrome>
  );
}
