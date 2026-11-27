import { useEffect, useRef } from "react";
import { data } from "react-router";

import { loadInviteReferral } from "@/data/lib/catalyst/landings/referral.server";
import { ETH_ADDRESS_RE } from "@/data/lib/catalyst/format/address";
import type { ReferrerVM } from "@/data/lib/catalyst/landings/referral";
import { sidLoader } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";

import { faceUrl } from "@/features/social/community/view";
import { InvitePage } from "@/pages/invite/InvitePage";

import {
  DEFAULT_SOCIAL_DESCRIPTION,
  DEFAULT_SOCIAL_TITLE,
  SOCIAL_LOGO,
  absoluteImage,
  absoluteUrl,
  publicBase,
  socialMeta,
} from "../lib/social-meta";
import type { Route } from "./+types/invite.($referrer)";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "landings/invite-referral";

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
    extra: [{ property: "og:logo", content: SOCIAL_LOGO }],
  });
}

export async function loader({ request, context, params }: Route.LoaderArgs) {
  const referrerParam = params.referrer?.trim() ?? "";

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
    referrerParam,
    referrer: load?.referrer ?? null,
  });
}

export default function InviteReferral({ loaderData }: Route.ComponentProps) {
  const { sid, referrerParam, referrer } = loaderData;
  useInviteTelemetry(sid, Boolean(referrerParam), referrer);
  return <InvitePage referrerName={referrer?.name ?? null} referrerAddress={referrer?.address ?? null} />;
}

function useInviteTelemetry(sid: string, hasReferrer: boolean, referrer: ReferrerVM | null) {
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
}
