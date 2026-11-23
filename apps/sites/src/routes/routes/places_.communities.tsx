import { usePageView } from "@/core/lib/telemetry/page-view";
import { COMMUNITIES_PAGE_VIEW } from "@/features/places/page-view";

import SocialRoute, { meta as socialMeta } from "./social";

/** decentraland.org's Communities tab of Places: the communities list, which lives on /social here. */
export const meta = socialMeta;

export default function PlacesCommunities() {
  // `/places/*` owns its page views (the root sends none there), as the legacy tab did.
  usePageView(COMMUNITIES_PAGE_VIEW);
  return <SocialRoute />;
}
