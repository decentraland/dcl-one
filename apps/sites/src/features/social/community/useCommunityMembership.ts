import { skipToken, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router";

import type { AuthIdentity } from "@/data/lib/auth/types";
import {
  cancelJoinRequest,
  joinCommunity,
  requestToJoin,
  type Community,
  type JoinRequest,
} from "@/data/lib/catalyst/social/community-detail";
import {
  COMMUNITY_STALE_MS,
  communityKeys,
  joinRequestsQuery,
  viewerCommunityQuery,
} from "@/data/lib/community/queries";

import { openSignIn } from "../../components/auth/signin-store";
import {
  ACTION_PARAM,
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
  viewer: Community | undefined;
  pending: boolean;
  /** The last write failed; the member's standing is what it was before it. */
  failed: boolean;
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
export function useCommunityMembership(community: Community, identity: AuthIdentity | null): Membership {
  const client = useQueryClient();
  const id = community.id;
  const viewer = useQuery(
    identity
      ? viewerCommunityQuery(identity, id)
      : { queryKey: communityKeys.viewer("", id), queryFn: skipToken, staleTime: COMMUNITY_STALE_MS },
  );
  const wantsRequests = Boolean(identity) && community.privacy === "private";
  const requests = useQuery(
    identity && wantsRequests
      ? joinRequestsQuery(identity)
      : { queryKey: communityKeys.requests(""), queryFn: skipToken, staleTime: COMMUNITY_STALE_MS },
  );
  const [failed, setFailed] = useState(false);

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
    onMutate: () => setFailed(false),
    onError: () => setFailed(true),
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
      await Promise.all([
        client.invalidateQueries({ queryKey: communityKeys.viewer(signer, id) }),
        client.invalidateQueries({ queryKey: communityKeys.detail(id) }),
        client.invalidateQueries({ queryKey: communityKeys.members(id) }),
        client.invalidateQueries({ queryKey: communityKeys.viewerMembers(signer, id) }),
        client.invalidateQueries({ queryKey: communityKeys.requests(signer) }),
      ]);
    },
  });

  const state = membershipState({
    signedIn: Boolean(identity),
    viewer: viewer.data,
    privacy: community.privacy,
    requests: wantsRequests ? requests.data : [],
    communityId: id,
  });

  const run = (kind: "join" | "request") => {
    if (identity) mutation.mutate({ kind, identity });
  };
  const signIn = () => {
    const action: CommunityAction = community.privacy === "private" ? "requestToJoin" : "join";
    openSignIn({ redirectTo: signInReturnHref(id, action) });
  };

  usePendingAction(state, mutation.isPending, (action) => run(action === "join" ? "join" : "request"));

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
  };
}

/**
 * `?action=join|requestToJoin`, set by the sign-in this page started: once the member's standing is
 * known, the action runs if it still applies, and the parameter is dropped either way.
 */
function usePendingAction(state: MembershipState, busy: boolean, perform: (action: CommunityAction) => void) {
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const done = useRef(false);
  const raw = params.get(ACTION_PARAM);
  const action = readAction(raw);
  useEffect(() => {
    if (raw === null || done.current) return;
    const drop = () => {
      done.current = true;
      const next = new URLSearchParams(params);
      next.delete(ACTION_PARAM);
      const query = next.toString();
      void navigate({ search: query ? `?${query}` : "" }, { replace: true, preventScrollReset: true });
    };
    if (!action) return drop();
    if (state.kind === "signed-out" || state.kind === "loading" || busy) return;
    if (action === "join" && state.kind === "join") perform("join");
    if (action === "requestToJoin" && state.kind === "request") perform("requestToJoin");
    drop();
  }, [raw, action, state.kind, busy, params, navigate, perform]);
}
