import { sendLegacyNow, type LegacySession } from "@/core/lib/telemetry/legacy-click";
import { track } from "@/core/lib/telemetry/track";

// The legacy community page's membership calls (decentraland/sites `CommunityDetail/CommunityInfo/CommunityInfo.tsx`),
// sent as it did: straight to analytics, so nothing leaves before the document's analytics is ready.

/** Which membership control was pressed. */
export type CommunityClick = "join" | "request" | "cancel" | "signIn";

/**
 * `userAddress` is the signed-in wallet, lowercased as the legacy wallet store kept it; a visitor with none
 * sends no `userAddress` at all.
 */
export function trackCommunityClick(
  click: CommunityClick,
  communityId: string,
  userAddress: string | undefined,
  session: Omit<LegacySession, "scope">,
): void {
  const props = { communityId, ...(userAddress ? { userAddress: userAddress.toLowerCase() } : {}) };
  sendLegacyNow((ctx) => {
    if (click === "join") track("Click on Join", props, ctx);
    else if (click === "request") track("Click on Request to Join", props, ctx);
    else if (click === "cancel") track("Click on Cancel Request", props, ctx);
    else track("Click on Sign In to Join", props, ctx);
  }, session);
}
