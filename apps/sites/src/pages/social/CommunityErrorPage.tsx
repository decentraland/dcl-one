import { useParams, useRouteError } from "react-router";

import { CommunityError } from "@dcl/one-ui";

import { communityKeys } from "@/data/lib/community/queries";
import SitesLayout from "@/features/components/chrome/SitesLayout";
import { useFailedReadRecovery } from "@/features/components/useFailedReadRecovery";
import { communityErrorReason, isCommunityError } from "@/features/social/community/errors";

/**
 * The document request's 404/503 response and a client navigation's failed community read render the
 * same screen. Retry resets the failed read and reloads the route; leaving releases it, so coming back
 * reads again.
 */
export function CommunityErrorPage() {
  const error = useRouteError();
  const params = useParams();
  const id = params.id ?? "";
  // A bug is the root boundary's to show, not a community failure.
  if (!isCommunityError(error)) throw error;
  const retry = useFailedReadRecovery(communityKeys.detail(id));
  return (
    <SitesLayout active="play" overlayNav>
      <CommunityError reason={communityErrorReason(error)} onRetry={retry} />
    </SitesLayout>
  );
}
