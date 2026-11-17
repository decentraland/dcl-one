import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { useLocation, useNavigate, useParams, useRouteError } from "react-router";

import SitesChrome from "@ui/web/frames/SitesChrome";

import { Button } from "@dcl/one-ui";
import { GeneralWarning } from "@dcl/one-ui/foundations/icons";

import { communityKeys } from "@/data/lib/community/queries";

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

const failedWithoutData = ({ state }: { state: { status: string; data: unknown } }) =>
  state.status === "error" && state.data === undefined;

/**
 * The document request's 404/503 response and a client navigation's failed community read render the
 * same screen. Retry resets the failed read and reloads the route; leaving releases it, so coming back
 * reads again.
 */
export function CommunityErrorView() {
  const error = useRouteError();
  const params = useParams();
  const client = useQueryClient();
  const navigate = useNavigate();
  const location = useLocation();
  const id = params.id ?? "";
  // A bug is the root boundary's to show, not a community failure.
  if (!isCommunityError(error)) throw error;
  const reason = communityErrorReason(error);
  const copy = COPY[reason];

  useEffect(
    () => () => {
      void client.resetQueries({
        queryKey: communityKeys.detail(id),
        exact: true,
        type: "inactive",
        predicate: (query) => failedWithoutData(query) && query.state.fetchStatus === "idle",
      });
    },
    [client, id],
  );

  const retry = async () => {
    await client.resetQueries({ queryKey: communityKeys.detail(id), exact: true, predicate: failedWithoutData });
    // A navigation to the same URL clears the router's error, which a revalidation does not for an
    // error thrown while rendering.
    void navigate(`${location.pathname}${location.search}`, { replace: true });
  };

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
