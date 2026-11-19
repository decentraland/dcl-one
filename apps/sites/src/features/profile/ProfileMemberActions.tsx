import { Fragment, useState, type ReactNode } from "react";

import { Button } from "@dcl/one-ui";
import { GeneralAdd, GeneralCheck, GeneralForbidden, GeneralLink, GeneralPlayers } from "@dcl/one-ui/foundations/icons";

import { useAuth } from "@/data/lib/auth/index";
import { normalizeAddress } from "@/data/lib/catalyst/social/profile";
import type { Friendship, FriendshipAction } from "@/data/lib/friends/friends";
import { signsFor } from "@/data/lib/profile/queries";

import { openSignIn } from "../components/auth/signin-store";
import { useCopyStatus } from "../components/useCopyStatus";
import {
  useBlocking,
  useChangeFriendship,
  useFriendsCount,
  useFriendship,
  useMutualFriendsCount,
  useSetBlocked,
} from "../friends/hooks";

import styles from "./ProfileMemberActions.module.css";
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

/** The signed-in member's own friends count, beside the owner's call to action. */
export function ProfileFriendsCount({ address }: { address: string }) {
  const { identity } = useAuth();
  const count = useFriendsCount(signsFor(identity, address) ? identity : null).data;
  if (count === undefined) return null;
  return (
    <span className={styles.label}>
      <GeneralPlayers {...ICON} />
      {count === 1 ? "1 Friend" : `${count} Friends`}
    </span>
  );
}

/** The link a profile is shared by: its canonical page on this site. */
export function profileShareUrl(address: string): string {
  const origin = typeof window === "undefined" ? "" : window.location.origin;
  return `${origin}${profileHref(address)}`;
}

/**
 * Another member's header actions: the friends the visitor shares with them, the friendship
 * control in its current state, block or unblock, and share. A signed-out visitor sees Add friend,
 * which opens sign-in, and share.
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
  /** A friendship button: it shows the write it started, and no second write starts meanwhile. */
  const action = (run: FriendshipAction, label: string, props: { primary?: boolean; icon?: ReactNode; name?: string } = {}) => (
    <Button
      variant={props.primary ? "primary" : "secondary"}
      size="md"
      leadingIcon={props.icon}
      aria-label={props.name}
      loading={change.isPending && change.variables === run}
      disabled={busy && !(change.isPending && change.variables === run)}
      onClick={() => act(run)}
    >
      {label}
    </Button>
  );

  let friend: ReactNode;
  if (!identity) {
    friend = (
      <Button size="md" leadingIcon={<GeneralAdd {...ICON} />} onClick={() => openSignIn({ inPlace: true })}>
        Add friend
      </Button>
    );
  } else if (state === undefined) {
    friend =
      friendshipFailed ? (
        <>
          <span className={styles.label}>Friendship unavailable</span>
          {tryAgain}
        </>
      ) : (
        <Button size="md" loading aria-label="Loading friendship">
          Add friend
        </Button>
      );
  } else if (state === "request_sent") {
    friend = (
      <>
        <span className={styles.label}>Request sent</span>
        {action("cancel", "Cancel request")}
      </>
    );
  } else if (state === "request_received") {
    friend = (
      <>
        {action("accept", "Accept", { primary: true, icon: <GeneralCheck {...ICON} />, name: "Accept friend request" })}
        {action("reject", "Reject", { name: "Reject friend request" })}
      </>
    );
  } else if (state === "friends") {
    friend = (
      <>
        <span className={styles.label}>
          <GeneralCheck {...ICON} />
          Friends
        </span>
        {action("remove", "Unfriend")}
      </>
    );
  } else if (state === "blocked") {
    friend = <span className={styles.label}>Blocked</span>;
  } else if (state === "blocked_by") {
    // A member who blocked the visitor takes no friend request.
    friend = null;
  } else {
    friend = action("request", "Add friend", { primary: true, icon: <GeneralAdd {...ICON} /> });
  }

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
        {!identity ? null : blockingFailed ? (
          // Without the block lists the button cannot say Block or Unblock. One Try again serves both failed reads.
          friendshipFailed ? null : (
            <>
              <span className={styles.label}>Block unavailable</span>
              {tryAgain}
            </>
          )
        ) : (
          <Button
            variant="secondary"
            size="md"
            leadingIcon={<GeneralForbidden {...ICON} />}
            loading={block.isPending}
            disabled={change.isPending || blocking.data === undefined}
            onClick={() => setBlock(!blocked)}
          >
            {blocked ? "Unblock" : "Block"}
          </Button>
        )}
        <Button
          variant="secondary"
          size="md"
          leadingIcon={<GeneralLink {...ICON} />}
          onClick={() => {
            setNotice(null);
            share.copy(profileShareUrl(address));
          }}
        >
          Share
        </Button>
      </div>
      {/* The status region stays mounted, so a screen reader hears each result; an alert is announced when it appears. */}
      <p className={styles.notice} role="status">
        {notice?.kind === "status" ? (
          notice.text
        ) : (
          <Fragment key={share.announcement}>
            {share.state === "copied" ? "Profile link copied" : share.state === "failed" ? "Couldn't copy the link" : null}
          </Fragment>
        )}
      </p>
      {notice?.kind === "error" ? (
        <p className={styles.error} role="alert">
          {notice.text}
        </p>
      ) : null}
    </div>
  );
}
