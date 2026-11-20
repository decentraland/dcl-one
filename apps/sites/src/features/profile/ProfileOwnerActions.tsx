import { useState } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { Link } from "react-router";

import { Button, Modal, ProfilePic, Spinner } from "@dcl/one-ui";
import { GeneralPlayers, MenusShare } from "@dcl/one-ui/foundations/icons";

import { useAuth } from "@/data/lib/auth/index";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { shortAddress } from "@/data/lib/catalyst/format/address";
import { referralInviteUrl } from "@/data/lib/catalyst/social/referrals";
import { friendProfilesQuery, friendsListQuery } from "@/data/lib/friends/queries";
import { signsFor } from "@/data/lib/profile/queries";

import { useCopyStatus } from "../components/useCopyStatus";
import { useFriendsCount } from "../friends/hooks";
import styles from "./ProfileOwnerActions.module.css";
import { profileHref } from "./tabs";

function FriendsList({ identity, close }: { identity: AuthIdentity; close: () => void }) {
  const list = useInfiniteQuery(friendsListQuery(identity));
  const addresses = [...new Set(list.data?.pages.flatMap((page) => page.addresses) ?? [])];
  const profiles = useQuery(friendProfilesQuery(identity, addresses));
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
            const profile = profiles.data?.[address];
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

export function ProfileInviteFriends({ address }: { address: string }) {
  const { identity } = useAuth();
  const { state, announcement, copy } = useCopyStatus();
  if (!signsFor(identity, address)) return null;
  return (
    <>
      <Button
        variant="outline"
        size="md"
        leadingIcon={<MenusShare size={18} aria-hidden="true" />}
        onClick={() => copy(referralInviteUrl(window.location.origin, address))}
      >
        {state === "copied" ? "Invite Copied" : "Invite Friends"}
      </Button>
      <span className={styles.announcement} role="status">
        <span key={announcement}>{state === "copied" ? "Invite link copied." : state === "failed" ? "Couldn't copy the invite link." : ""}</span>
      </span>
    </>
  );
}
