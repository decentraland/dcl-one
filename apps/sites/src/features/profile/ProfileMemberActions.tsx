import { useState } from "react";

import {
  ProfileMemberActionControls,
  type MemberActionNotice,
} from "@dcl/one-ui/apps/sites/profile/ProfileMemberActionControls";

import { useAuth } from "@/data/lib/auth/index";
import { normalizeAddress } from "@/data/lib/catalyst/social/profile";
import type { Friendship, FriendshipAction } from "@/data/lib/friends/friends";

import { openSignIn } from "../components/auth/signin-store";
import {
  useBlocking,
  useChangeFriendship,
  useFriendship,
  useMutualFriendsCount,
  useSetBlocked,
} from "../friends/hooks";

import { profileHref } from "./tabs";

/** What the page says after each friendship write, for sighted and screen reader users alike. */
const DONE: Record<FriendshipAction, (name: string) => string> = {
  request: () => "Friend request sent.",
  cancel: () => "Friend request cancelled.",
  accept: (name) => `You and ${name} are now friends.`,
  reject: () => "Friend request rejected.",
  remove: (name) => `${name} is no longer your friend.`,
};

/** The link a profile is shared by: its canonical page on this site. */
export function profileShareUrl(address: string): string {
  const origin = typeof window === "undefined" ? "" : window.location.origin;
  return `${origin}${profileHref(address)}`;
}

/**
 * Another member's header actions: the friends the visitor shares with them, the friendship
 * control in its current state, and the More actions menu with block or unblock, and share. A
 * signed-out visitor sees Add friend, which opens sign-in, and a menu with share only.
 */
export function ProfileMemberActions({ address: rawAddress, name }: { address: string; name: string }) {
  const address = normalizeAddress(rawAddress);
  const { identity } = useAuth();
  const friendship = useFriendship(identity, address);
  const blocking = useBlocking(identity);
  const mutual = useMutualFriendsCount(identity, address).data;
  const change = useChangeFriendship(identity, address);
  const block = useSetBlocked(identity, address);
  const [notice, setNotice] = useState<MemberActionNotice | null>(null);

  // The block lists decide a block either way; the friendship read only knows one without a friendship.
  const blocked = blocking.data?.blocked.includes(address) === true || friendship.data === "blocked";
  const blockedBy = blocking.data?.blockedBy.includes(address) === true || friendship.data === "blocked_by";
  const state: Friendship | undefined = blocked ? "blocked" : blockedBy ? "blocked_by" : friendship.data;
  // A read that failed with nothing to show: the friendship (no state to act on) or the block lists (Block cannot tell its side).
  const friendshipFailed = state === undefined && friendship.isError;
  const blockingFailed = blocking.data === undefined && blocking.isError;
  const retrying = (friendshipFailed && friendship.isFetching) || (blockingFailed && blocking.isFetching);
  /** One Try again reads every failed read again. */
  const retry = () => {
    if (friendshipFailed) void friendship.refetch();
    if (blockingFailed) void blocking.refetch();
  };
  const act = (action: FriendshipAction) => {
    setNotice(null);
    change.mutate(action, {
      onSuccess: () => setNotice({ kind: "status", text: DONE[action](name) }),
      onError: () => setNotice({ kind: "error", text: "The friendship could not be updated. Try again." }),
    });
  };
  const setBlock = (next: boolean) => {
    setNotice(null);
    block.mutate(next, {
      onSuccess: () => setNotice({ kind: "status", text: `${name} has been ${next ? "blocked" : "unblocked"}.` }),
      onError: () =>
        setNotice({ kind: "error", text: `${name} could not be ${next ? "blocked" : "unblocked"}. Try again.` }),
    });
  };

  return (
    <ProfileMemberActionControls
      signedIn={Boolean(identity)}
      state={state}
      friendshipFailed={friendshipFailed}
      blockingFailed={blockingFailed}
      blockListReady={blocking.data !== undefined}
      retrying={retrying}
      mutual={mutual}
      pendingAction={change.isPending ? change.variables : undefined}
      blockingPending={block.isPending}
      shareUrl={profileShareUrl(address)}
      notice={notice}
      retry={retry}
      act={act}
      setBlock={setBlock}
      onSignIn={() => openSignIn({ inPlace: true })}
      onDismissNotice={() => setNotice(null)}
    />
  );
}
