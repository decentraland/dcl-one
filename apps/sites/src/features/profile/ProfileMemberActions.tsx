import { Fragment, useState, type ReactNode } from "react";

import { Button, FriendCTA, Toast, type FriendCTAStatus } from "@dcl/one-ui";
import { GeneralCheck, GeneralForbidden, GeneralPlayers, MenusShare } from "@dcl/one-ui/foundations/icons";

import { useAuth } from "@/data/lib/auth/index";
import { normalizeAddress } from "@/data/lib/catalyst/social/profile";
import type { Friendship, FriendshipAction } from "@/data/lib/friends/friends";

import { openSignIn } from "../components/auth/signin-store";
import { useCopyStatus } from "../components/useCopyStatus";
import {
  useBlocking,
  useChangeFriendship,
  useFriendship,
  useMutualFriendsCount,
  useSetBlocked,
} from "../friends/hooks";

import styles from "./ProfileMemberActions.module.css";
import { ProfileMoreActions, type MoreAction } from "./ProfileMoreActions";
import { profileHref } from "./tabs";

const ICON = { size: 18, "aria-hidden": true } as const;

/** What the page says after each friendship write, for sighted and screen reader users alike. */
const DONE: Record<FriendshipAction, (name: string) => string> = {
  request: () => "Friend request sent.",
  cancel: () => "Friend request cancelled.",
  accept: (name) => `You and ${name} are now friends.`,
  reject: () => "Friend request rejected.",
  remove: (name) => `${name} is no longer your friend.`,
};

type Notice = { kind: "status" | "error"; text: string };

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
  const share = useCopyStatus();
  const [notice, setNotice] = useState<Notice | null>(null);

  // The block lists decide a block either way; the friendship read only knows one without a friendship.
  const blocked = blocking.data?.blocked.includes(address) === true || friendship.data === "blocked";
  const blockedBy = blocking.data?.blockedBy.includes(address) === true || friendship.data === "blocked_by";
  const state: Friendship | undefined = blocked ? "blocked" : blockedBy ? "blocked_by" : friendship.data;
  const busy = change.isPending || block.isPending;
  // A read that failed with nothing to show: the friendship (no state to act on) or the block lists (Block cannot tell its side).
  const friendshipFailed = state === undefined && friendship.isError;
  const blockingFailed = blocking.data === undefined && blocking.isError;
  const retrying = (friendshipFailed && friendship.isFetching) || (blockingFailed && blocking.isFetching);
  /** One Try again reads every failed read again. */
  const retry = () => {
    if (friendshipFailed) void friendship.refetch();
    if (blockingFailed) void blocking.refetch();
  };
  const tryAgain = (
    <Button variant="secondary" size="md" loading={retrying} onClick={retry}>
      Try again
    </Button>
  );

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
      onError: () => setNotice({ kind: "error", text: `${name} could not be ${next ? "blocked" : "unblocked"}. Try again.` }),
    });
  };
  /** Whether this friendship write is the one in progress; no second write starts meanwhile. */
  const running = (run: FriendshipAction) => change.isPending && change.variables === run;
  const cta = (status: FriendCTAStatus, run: FriendshipAction, name?: string) => (
    <FriendCTA status={status} aria-label={name} loading={running(run)} disabled={busy && !running(run)} onClick={() => act(run)} />
  );

  let friend: ReactNode;
  if (!identity) {
    friend = <FriendCTA status="add" onClick={() => openSignIn({ inPlace: true })} />;
  } else if (state === undefined) {
    friend = friendshipFailed ? (
      <>
        <span className={styles.label}>Friendship unavailable</span>
        {tryAgain}
      </>
    ) : (
      <FriendCTA status="add" loading aria-label="Loading friendship" />
    );
  } else if (state === "request_sent") {
    friend = cta("sent", "cancel");
  } else if (state === "request_received") {
    friend = (
      <>
        {/* The design system's friendship button has no Accept state. */}
        <Button
          size="md"
          leadingIcon={<GeneralCheck {...ICON} />}
          aria-label="Accept friend request"
          loading={running("accept")}
          disabled={busy && !running("accept")}
          onClick={() => act("accept")}
        >
          Accept
        </Button>
        {cta("reject", "reject", "Reject friend request")}
      </>
    );
  } else if (state === "friends") {
    friend = cta("friend", "remove");
  } else if (state === "blocked") {
    // Unblock is the action of the blocked state; without the block lists it cannot run.
    friend = (
      <FriendCTA
        status="blocked"
        loading={block.isPending}
        disabled={change.isPending || blocking.data === undefined}
        onClick={() => setBlock(false)}
      />
    );
  } else if (state === "blocked_by") {
    // A member who blocked the visitor takes no friend request.
    friend = null;
  } else {
    friend = cta("add", "request");
  }

  /** The menu's items. Without the block lists it cannot say Block or Unblock; a signed-out visitor has no block. */
  const more: MoreAction[] = [
    ...(identity && !blockingFailed
      ? [
          {
            label: blocked ? "Unblock" : "Block",
            icon: <GeneralForbidden {...ICON} />,
            disabled: busy || blocking.data === undefined,
            onSelect: () => setBlock(!blocked),
          },
        ]
      : []),
    {
      label: "Share",
      icon: <MenusShare {...ICON} />,
      onSelect: () => {
        setNotice(null);
        share.copy(profileShareUrl(address));
      },
    },
  ];

  return (
    <div className={styles.root}>
      <div className={styles.controls}>
        {/* Members who blocked one another see no shared friends. */}
        {mutual !== undefined && mutual > 0 && !blocked && !blockedBy ? (
          <span className={styles.label}>
            <GeneralPlayers {...ICON} />
            {`${mutual} Mutual`}
          </span>
        ) : null}
        {friend}
        {/* One Try again serves both failed reads. */}
        {identity && blockingFailed && !friendshipFailed ? (
          <>
            <span className={styles.label}>Block unavailable</span>
            {tryAgain}
          </>
        ) : null}
        <ProfileMoreActions actions={more} />
      </div>
      {/* The status region stays mounted, so a screen reader hears each result. An alert is announced when it appears, and stays until it is closed. */}
      <Toast onClose={() => setNotice(null)}>
        {notice?.kind === "status" ? (
          notice.text
        ) : share.state === "idle" ? null : (
          <Fragment key={share.announcement}>{share.state === "copied" ? "Profile link copied" : "Couldn't copy the link"}</Fragment>
        )}
      </Toast>
      {notice?.kind === "error" ? (
        <Toast role="alert" variant="error" duration={0} closeButton onClose={() => setNotice(null)}>
          {notice.text}
        </Toast>
      ) : null}
    </div>
  );
}
