import { useState } from "react";
import { useInfiniteQuery, useQueries } from "@tanstack/react-query";
import { Link } from "react-router";

import { Button, CopyFeedback, Modal, ProfilePic, Spinner } from "@dcl/one-ui";
import { GeneralPlayers, MenusShare } from "@dcl/one-ui/foundations/icons";

import { useAuth } from "@/data/lib/auth/index";
import type { AuthIdentity } from "@/data/lib/auth/types";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";
import { shortAddress } from "@/data/lib/catalyst/format/address";
import { profileReferralHandle, referralInviteUrl } from "@/data/lib/catalyst/social/referrals";
import { friendProfilesQuery, friendsListQuery } from "@/data/lib/friends/queries";
import { signsFor } from "@/data/lib/profile/queries";

import { useFriendsCount } from "../friends/hooks";
import { useProfileFlags } from "./hooks";
import styles from "./ProfileOwnerActions.module.css";
import { profileHref } from "./tabs";

function FriendsList({ identity, close }: { identity: AuthIdentity; close: () => void }) {
  const list = useInfiniteQuery(friendsListQuery(identity));
  const addresses = [...new Set(list.data?.pages.flatMap((page) => page.addresses) ?? [])];
  const profilePages = useQueries({
    queries: list.data?.pages.map((page) => friendProfilesQuery(identity, page.addresses)) ?? [],
  });
  const profiles = Object.assign({}, ...profilePages.map((page) => page.data ?? {})) as NonNullable<(typeof profilePages)[number]["data"]>;
  const failed = list.isError && !list.data;
  const nextFailed = list.isFetchNextPageError;
  return (
    <Modal title="Friends" onClose={close}>
      {list.isPending ? <Spinner label="Loading friends" /> : null}
      {failed ? (
        <div role="status">
          <p>Friends are unavailable right now.</p>
          <Button variant="outline" size="sm" onClick={() => void list.refetch()}>Try again</Button>
        </div>
      ) : null}
      {list.data && addresses.length === 0 ? <p>You don't have any friends yet.</p> : null}
      {addresses.length > 0 ? (
        <ul className={styles.friends}>
          {addresses.map((address) => {
            const profile = profiles[address];
            const name = profile?.name || shortAddress(address);
            return (
              <li key={address}>
                <Link to={profileHref(address)} className={styles.friend} onClick={close}>
                  <ProfilePic name={name} src={profile?.picture ?? undefined} size={32} alt="" />
                  {name}
                </Link>
              </li>
            );
          })}
        </ul>
      ) : null}
      {list.hasNextPage ? (
        <div className={styles.more}>
          {nextFailed ? <p role="status">More friends could not be loaded.</p> : null}
          <Button variant="outline" size="sm" loading={list.isFetchingNextPage} onClick={() => void list.fetchNextPage()}>
            {nextFailed ? "Try again" : "Load more"}
          </Button>
        </div>
      ) : null}
    </Modal>
  );
}

export function ProfileFriendsCount({ address }: { address: string }) {
  const { identity } = useAuth();
  const ownIdentity = signsFor(identity, address) ? identity : null;
  const count = useFriendsCount(ownIdentity).data;
  const [open, setOpen] = useState(false);
  if (!ownIdentity || count === undefined) return null;
  return (
    <>
      <Button variant="outline" size="md" leadingIcon={<GeneralPlayers size={18} aria-hidden="true" />} onClick={() => setOpen(true)}>
        {count === 1 ? "1 Friend" : `${count} Friends`}
      </Button>
      {open ? <FriendsList identity={ownIdentity} close={() => setOpen(false)} /> : null}
    </>
  );
}

export function ProfileInviteFriends({ profile }: { profile: ProfileVM }) {
  const { identity } = useAuth();
  const own = signsFor(identity, profile.address);
  const flags = useProfileFlags(own);
  if (!own || flags.data?.referral !== true) return null;
  const origin = typeof window === "undefined" ? "" : window.location.origin;
  return (
    <CopyFeedback
      value={referralInviteUrl(origin, profileReferralHandle(profile))}
      successAnnouncement="Invite link copied."
      errorAnnouncement="Couldn't copy the invite link."
      showTooltip={false}
    >
      {(copy, state) => (
        <Button
          variant="outline"
          size="md"
          leadingIcon={<MenusShare size={18} aria-hidden="true" />}
          onClick={copy}
        >
          {state === "copied" ? "Invite Copied" : state === "failed" ? "Copy failed" : "Invite Friends"}
        </Button>
      )}
    </CopyFeedback>
  );
}
