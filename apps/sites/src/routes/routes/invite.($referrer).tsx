import { useEffect, useRef } from "react";
import { useSearchParams } from "react-router";

import LdInviteReferralView from "@ui/landings/pages/LdInviteReferralView";

import { loadInviteReferral } from "@/data/lib/catalyst/landings/referral.server";
import type { ReferrerVM, ReferralState } from "@/data/lib/catalyst/landings/referral";
import { pageLoader, type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/invite.($referrer)";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "landings/invite-referral";

type Faq = { q: string; a: string };
type Step = "view" | "download" | "rewards" | "faqs";

const parseStep = (raw: string | null): Step => (raw === "download" || raw === "rewards" || raw === "faqs" ? raw : "view");

const FALLBACK: PageDefaults = { variant: "personalized_referrer" };

// from: sites@17b3535a:src/pages/invite/InvitePage.tsx useDocumentMeta and src/intl/en.json page_invite.social.
export function meta({ loaderData }: Route.MetaArgs) {
  const name = loaderData?.referrer?.name;
  return [
    { title: name ? `${name} invited you to join Decentraland` : "Join Decentraland - A Virtual World Built by Its Community" },
    {
      name: "description",
      content:
        "Decentraland is where you hang out online. Join movie screenings, live music, trivia, and other community events. Show up anytime and see who's around.",
    },
  ];
}

export async function loader({ request, context, params }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const referrerParam = params.referrer?.trim() ?? "";
  const step = parseStep(url.searchParams.get("step"));

  const { sid, wrap } = await pageLoader(request, context, FALLBACK);

  const load = await loadInviteReferral(referrerParam, request.signal).catch(() => null);
  const attribution = load?.referrerParam ? { simulated: true as const, referrer: load.referrer.address ?? load.referrer.name, session: sid } : null;
  return wrap({
    sid,
    step,
    referrerParam,
    referrer: load?.referrer ?? null,
    referral: load?.referral ?? null,
    faqs: (load?.faqs ?? []) as Faq[],
    isFixture: load?.isFixture ?? true,
    attribution,
  });
}

export default function InviteReferral({ loaderData }: Route.ComponentProps) {
  return <InviteLanding {...loaderData} />;
}

function InviteLanding({ sid, step, referrerParam, referrer, referral }: { sid: string; step: Step; referrerParam: string; referrer: ReferrerVM | null; referral: ReferralState | null }) {
  const [, setSearchParams] = useSearchParams();
  const hasReferrer = Boolean(referrerParam);
  useInviteTelemetry(sid, step, hasReferrer, referrer, referral);
  return (
    <LdInviteReferralView
      step={step}
      handle={referrer?.name ?? null}
      referrerAddress={referrer?.address ?? null}
      referrerHasClaimedName={referrer?.hasClaimedName ?? false}
      invitedUsersAccepted={referral?.invitedUsersAccepted ?? 0}
      onStep={(s) =>
        setSearchParams((prev) => {
          const next = new URLSearchParams(prev);
          if (s === "view") next.delete("step");
          else next.set("step", s);
          return next;
        }, { preventScrollReset: false })
      }
    />
  );
}

function useInviteTelemetry(sid: string, step: Step, hasReferrer: boolean, referrer: ReferrerVM | null, referral: ReferralState | null) {
  const viewed = useRef(false);
  useEffect(() => {
    if (viewed.current) return;
    viewed.current = true;
    track("lp_invite_viewed", { has_referrer: hasReferrer, resolved: referrer?.resolved ?? false, referrer_handle: referrer?.name ?? null }, { sid, story: STORY });
  }, [sid, hasReferrer, referrer]);

  const resolvedFired = useRef(false);
  useEffect(() => {
    if (resolvedFired.current || !hasReferrer || !referrer) return;
    resolvedFired.current = true;
    track(
      "lp_invite_referrer_resolved",
      { referrer_handle: referrer.name ?? referrer.address ?? "", resolved: referrer.resolved, source: referrer.resolved ? "profile_lambdas" : "param" },
      { sid, story: STORY },
    );
  }, [sid, hasReferrer, referrer]);

  const rewardsFired = useRef(false);
  useEffect(() => {
    if (step !== "rewards" || rewardsFired.current || !referral) return;
    rewardsFired.current = true;
    track("lp_invite_rewards_viewed", { accepted: referral.invitedUsersAccepted, current_tier: referral.currentTier }, { sid, story: STORY });
  }, [sid, step, referral]);
}
