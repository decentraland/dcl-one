import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useLocation, useNavigate, useSearchParams } from "react-router";

import type { AuthIdentity } from "@/data/lib/auth/types";
import {
  cancelJoinRequest,
  joinCommunity,
  requestToJoin,
  type CommunityInfo,
  type JoinRequest,
} from "@/data/lib/catalyst/social/community-detail";
import {
  communityKeys,
  invalidateAfterMembershipChange,
  joinRequestsQuery,
  viewerCommunityQuery,
} from "@/data/lib/community/queries";

import { openSignIn } from "../../components/auth/signin-store";
import { rememberAction, takeAction } from "./pendingAction";
import {
  ACTION_PARAM,
  NONCE_PARAM,
  membershipState,
  readAction,
  signInReturnHref,
  type CommunityAction,
  type MembershipState,
} from "./view";

type Write =
  | { kind: "join"; identity: AuthIdentity }
  | { kind: "request"; identity: AuthIdentity }
  | { kind: "cancel"; identity: AuthIdentity; requestId: string };

export type Membership = {
  state: MembershipState;
  /** The community as the member reads it (with their role), once known. */
  viewer: CommunityInfo | undefined;
  pending: boolean;
  /** The write that last failed, if any; the member's standing is what it was before it. */
  failed: Write["kind"] | null;
  /** Reads the member's standing again after it failed. */
  retry: () => void;
  join: () => void;
  requestToJoin: () => void;
  cancelRequest: () => void;
  signIn: () => void;
};

/**
 * The signed-in member's standing in one community and the writes that change it: join a public
 * community, ask to join a private one, withdraw that request. Nothing changes in the cache until a
 * write succeeds, so a failed one leaves the page as it was. A sign-in started here comes back with
 * `?action=` and runs that action once.
 */
export function useCommunityMembership(community: CommunityInfo, identity: AuthIdentity | null): Membership {
  const client = useQueryClient();
  const id = community.id;
  const viewer = useQuery(viewerCommunityQuery(identity, id));
  const wantsRequests = Boolean(identity) && community.privacy === "private";
  const requests = useQuery(joinRequestsQuery(wantsRequests ? identity : null));
  // Keyed to the account that wrote, so a failure never shows under another account or signed out.
  const [failure, setFailure] = useState<{ kind: Write["kind"]; signer: string } | null>(null);
  const failed = failure && identity && failure.signer === identity.signer.toLowerCase() ? failure.kind : null;

  const mutation = useMutation({
    mutationFn: async (write: Write): Promise<JoinRequest | null> => {
      if (write.kind === "join") {
        await joinCommunity(write.identity, id);
        return null;
      }
      if (write.kind === "request") return requestToJoin(write.identity, id);
      await cancelJoinRequest(write.identity, id, write.requestId);
      return null;
    },
    onMutate: () => setFailure(null),
    onError: (_error, write) => setFailure({ kind: write.kind, signer: write.identity.signer.toLowerCase() }),
    onSuccess: async (created, write) => {
      const signer = write.identity.signer;
      if (write.kind === "request" && created) {
        client.setQueryData<JoinRequest[]>(communityKeys.requests(signer), (old) => [...(old ?? []), created]);
      }
      if (write.kind === "cancel") {
        client.setQueryData<JoinRequest[]>(communityKeys.requests(signer), (old) =>
          old?.filter((request) => request.id !== write.requestId),
        );
      }
      await invalidateAfterMembershipChange(client, signer, id, write.kind);
    },
  });

  const state = membershipState({
    signedIn: Boolean(identity),
    viewer: viewer.data,
    privacy: community.privacy,
    requests: wantsRequests ? requests.data : [],
    communityId: id,
    viewerFailed: viewer.isError,
    requestsFailed: wantsRequests && requests.isError,
  });

  const run = (kind: "join" | "request") => {
    if (identity) mutation.mutate({ kind, identity });
  };
  const signIn = () => {
    const action: CommunityAction = community.privacy === "private" ? "requestToJoin" : "join";
    openSignIn({ redirectTo: signInReturnHref(id, action, rememberAction(id, action)) });
  };

  usePendingAction(id, state, mutation.isPending, (action) => run(action === "join" ? "join" : "request"));

  return {
    state,
    viewer: viewer.data,
    pending: mutation.isPending,
    failed,
    join: () => run("join"),
    requestToJoin: () => run("request"),
    cancelRequest: () => {
      if (identity && state.kind === "cancel") mutation.mutate({ kind: "cancel", identity, requestId: state.requestId });
    },
    signIn,
    retry: () => {
      if (viewer.isError) void viewer.refetch();
      if (wantsRequests && requests.isError) void requests.refetch();
    },
  };
}

/**
 * `?action=join|requestToJoin&nonce=`, set by the sign-in this page started. Once the member's
 * standing is known the parameters are dropped, and the action runs only if this tab recorded it
 * (a matching, unexpired marker, consumed here) and it still applies. A link that carries the
 * parameters alone runs nothing: the member has to click.
 */
function usePendingAction(
  communityId: string,
  state: MembershipState,
  busy: boolean,
  perform: (action: CommunityAction) => void,
) {
  const [params] = useSearchParams();
  const { state: entryState } = useLocation();
  const navigate = useNavigate();
  const done = useRef(false);
  const raw = params.get(ACTION_PARAM);
  const nonce = params.get(NONCE_PARAM);
  const action = readAction(raw);
  useEffect(() => {
    if ((raw === null && nonce === null) || done.current) return;
    const drop = () => {
      done.current = true;
      const next = new URLSearchParams(params);
      next.delete(ACTION_PARAM);
      next.delete(NONCE_PARAM);
      const query = next.toString();
      // The entry keeps its state, such as the mark of an event detail opened meanwhile.
      void navigate(
        { search: query ? `?${query}` : "" },
        { replace: true, preventScrollReset: true, state: entryState },
      );
    };
    if (!action) return drop();
    if (state.kind === "signed-out" || state.kind === "loading" || busy) return;
    // A failed read of the member's standing drops the action: they retry and click themselves.
    const started = takeAction(communityId, action, nonce);
    if (started && action === "join" && state.kind === "join") perform("join");
    if (started && action === "requestToJoin" && state.kind === "request") perform("requestToJoin");
    drop();
  }, [communityId, raw, nonce, action, state.kind, busy, params, entryState, navigate, perform]);
}
