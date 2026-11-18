import { useParams, useRouteError } from "react-router";

import SitesChrome from "@ui/web/frames/SitesChrome";

import { Button } from "@dcl/one-ui";
import { GeneralWarning } from "@dcl/one-ui/foundations/icons";

import { communityKeys } from "@/data/lib/community/queries";

import { useFailedReadRecovery } from "../../components/useFailedReadRecovery";
import styles from "./CommunityErrorView.module.css";
import { communityErrorReason, isCommunityError } from "./errors";

const COPY = {
  "not-found": {
    title: "Community not found",
    body: "The community you are looking for does not exist.",
  },
  unavailable: {
    title: "Community unavailable",
    body: "We couldn't load this community right now. Try again in a moment.",
  },
} as const;

/**
 * The document request's 404/503 response and a client navigation's failed community read render the
 * same screen. Retry resets the failed read and reloads the route; leaving releases it, so coming back
 * reads again.
 */
export function CommunityErrorView() {
  const error = useRouteError();
  const params = useParams();
  const id = params.id ?? "";
  // A bug is the root boundary's to show, not a community failure.
  if (!isCommunityError(error)) throw error;
  const reason = communityErrorReason(error);
  const copy = COPY[reason];

  const retry = useFailedReadRecovery(communityKeys.detail(id));

  return (
    <SitesChrome active="play" overlayNav>
      <main className={styles.root} role="alert">
        <GeneralWarning className={styles.icon} size={72} aria-hidden="true" />
        <h1 className={styles.title}>{copy.title}</h1>
        <p className={styles.body}>{copy.body}</p>
        {reason === "unavailable" ? (
          <Button variant="secondary" size="md" onClick={retry}>
            Try again
          </Button>
        ) : null}
      </main>
    </SitesChrome>
  );
}
