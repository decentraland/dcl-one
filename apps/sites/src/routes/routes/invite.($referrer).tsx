import { useEffect, useRef } from "react";
import { data, useSearchParams } from "react-router";

import LdInviteReferralView from "@ui/landings/pages/LdInviteReferralView";

import { loadInviteReferral } from "@/data/lib/catalyst/landings/referral.server";
import { ETH_ADDRESS_RE } from "@/data/lib/catalyst/format/address";
import type { ReferrerVM, ReferralState } from "@/data/lib/catalyst/landings/referral";
import { sidLoader } from "@/core/lib/telemetry/page-loader";
import { useLegacyTrackScope } from "@/core/lib/telemetry/legacy-click";
import { useAnalyticsUserId } from "@/core/lib/telemetry/page-view";
import { track } from "@/core/lib/telemetry/track";

import { trackInviteClick } from "@/features/invite/invite-click";
import { faceUrl } from "@/features/social/community/view";

import { DEFAULT_SOCIAL_DESCRIPTION, DEFAULT_SOCIAL_TITLE, absoluteImage, absoluteUrl, publicBase, socialMeta } from "../lib/social-meta";
import type { Route } from "./+types/invite.($referrer)";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "landings/invite-referral";

type Step = "view" | "download" | "rewards" | "faqs";

const parseStep = (raw: string | null): Step => (raw === "download" || raw === "rewards" || raw === "faqs" ? raw : "view");

// The card names the referrer when the node knows the name, with their face as a small image.
export function meta({ loaderData, matches, location }: Route.MetaArgs) {
  const referrer = loaderData?.referrer;
  const param = loaderData?.referrerParam ?? "";
  const path = ETH_ADDRESS_RE.test(param) ? `/invite/${param.toLowerCase()}` : location.pathname;
  const base = publicBase(matches);
  return socialMeta({
    title: referrer?.name ? `${referrer.name} invited you to join Decentraland` : DEFAULT_SOCIAL_TITLE,
    description: DEFAULT_SOCIAL_DESCRIPTION,
    // Profile data anyone can write: only this site's profile-image or content paths, under the public base.
    image: absoluteImage(base, faceUrl(referrer?.face256)),
    smallImage: true,
    url: absoluteUrl(base, path),
  });
}

export async function loader({ request, context, params }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const referrerParam = params.referrer?.trim() ?? "";
  const step = parseStep(url.searchParams.get("step"));

  const { sid, wrap } = sidLoader(request, context);

  // An invite is someone's: no referrer, or one that names no wallet, is not a page.
  if (!referrerParam) throw data(null, { status: 404, statusText: "Not Found" });
  // A lookup that fails keeps the page, without a referrer; the log is what shows the node could not answer.
  const load = await loadInviteReferral(referrerParam, request.signal).catch((err: unknown) => {
    if (!request.signal.aborted) console.error(`[invite] referrer lookup failed: ${err instanceof Error ? err.message : String(err)}`);
    return null;
  });
  if (load && !load.referrer.address) throw data(null, { status: 404, statusText: "Not Found" });
  return wrap({
    sid,
    step,
    referrerParam,
    referrer: load?.referrer ?? null,
    referral: load?.referral ?? null,
  });
}

export default function InviteReferral({ loaderData }: Route.ComponentProps) {
  const { sid, step, referrerParam, referrer, referral } = loaderData;
  const [, setSearchParams] = useSearchParams();
  const userId = useAnalyticsUserId();
  const legacyScope = useLegacyTrackScope();
  const hasReferrer = Boolean(referrerParam);
  useInviteTelemetry(sid, step, hasReferrer, referrer, referral);
  return (
    <LdInviteReferralView
      step={step}
      handle={referrer?.name ?? null}
      referrerAddress={referrer?.address ?? null}
      referrerHasClaimedName={referrer?.hasClaimedName ?? false}
      invitedUsersAccepted={referral?.invitedUsersAccepted ?? 0}
      onCta={(hero) => trackInviteClick(hero, { sid, userId, scope: legacyScope })}
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
