import { useMemo } from "react";

import type { PhotoViewerEvents } from "@dcl/one-ui/apps/sites/profile/PhotoViewer";

import { sendLegacyDirect, trackLegacyClick, useLegacyTrackScope } from "@/core/lib/telemetry/legacy-click";
import { useAnalyticsSid, useAnalyticsUserId } from "@/core/lib/telemetry/page-view";
import { track } from "@/core/lib/telemetry/track";
import type { ProfilePhoto } from "@/data/lib/catalyst/social/photos";

/** decentraland.org's profile site, which `Reels Click Profile` names as `legacyProfileUrl` for a person in the photo. */
const LEGACY_PROFILE_URL = "https://profile.decentraland.org/accounts/";

/** An address as the camera reel service sent it, which the legacy events reported. */
const sent = (who: { address: string; sourceAddress?: string }) => who.sourceAddress ?? who.address;

function useReelsSession() {
  const sid = useAnalyticsSid();
  const userId = useAnalyticsUserId();
  return useMemo(() => ({ sid, userId }), [sid, userId]);
}

/** `Reels Click Thumbnail` on a gallery's photo (decentraland/sites `pages/reels/ReelsListPage.tsx`). */
export function useReelThumbnailClick(): (imageId: string) => void {
  const session = useReelsSession();
  return (imageId) => sendLegacyDirect((ctx) => track("Reels Click Thumbnail", { imageId }, ctx), session);
}

export type ReelEvents = {
  viewer: PhotoViewerEvents;
  onCopied: () => void;
  onDownload: () => void;
  onHomeClick: () => void;
};

/**
 * The events decentraland.org's Reels photo page sent (decentraland/sites `components/Reels/*`): `Reels *` through
 * the SDK directly, and the logo's `Click` through the deferred queue. `wearableUrn` names the URN of a person's
 * wearable as the viewer listed it.
 */
export function useReelEvents(
  photo: ProfilePhoto,
  wearableUrn: (person: number, wearable: number) => string | undefined,
): ReelEvents {
  const session = useReelsSession();
  const scope = useLegacyTrackScope();
  const imageId = photo.id;
  const { x, y } = photo.scene;
  const direct = (send: Parameters<typeof sendLegacyDirect>[0]) => sendLegacyDirect(send, session);

  return {
    viewer: {
      onShare: () => direct((ctx) => track("Reels Share", { imageId }, ctx)),
      onDetailsToggle: (visible) =>
        direct((ctx) =>
          visible ? track("Reels Show Metadata", { imageId }, ctx) : track("Reels Hide Metadata", { imageId }, ctx),
        ),
      onTakenByClick: () => direct((ctx) => track("Reels Click Profile", { userAddress: sent(photo.author) }, ctx)),
      onPlaceClick: () => direct((ctx) => track("Reels Click Place", { x, y }, ctx)),
      onJumpInClick: () => direct((ctx) => track("Reels Jump In", { x, y }, ctx)),
      onPersonClick: (index) => {
        const person = photo.people[index];
        if (!person) return;
        const userAddress = sent(person);
        direct((ctx) =>
          track("Reels Click Profile", { userAddress, legacyProfileUrl: `${LEGACY_PROFILE_URL}${userAddress}` }, ctx),
        );
      },
      onWearablesToggle: (index, open) => {
        const person = photo.people[index];
        if (!person) return;
        const userAddress = sent(person);
        direct((ctx) =>
          open ? track("Reels Show Wearables", { userAddress }, ctx) : track("Reels Hide Wearables", { userAddress }, ctx),
        );
      },
      onWearableClick: (index, wearable) => {
        const urn = wearableUrn(index, wearable);
        if (!urn) return;
        direct((ctx) => track("Reels Click Wearable", { wearableUrn: urn }, ctx));
      },
    },
    onCopied: () => direct((ctx) => track("Reels Copy Link", { imageId }, ctx)),
    onDownload: () => direct((ctx) => track("Reels Download", { imageId }, ctx)),
    onHomeClick: () => trackLegacyClick({ event: "Reels Click Decentraland Logo" }, { ...session, scope }),
  };
}
