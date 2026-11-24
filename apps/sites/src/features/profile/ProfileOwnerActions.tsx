import { useState } from "react";
import { useInfiniteQuery, useQueries } from "@tanstack/react-query";
import { Link } from "react-router";

import { ProfileFriendsButton, ProfileFriendsDialog, ProfileInviteButton } from "@dcl/one-ui/apps/sites";

import { useAuth } from "@/data/lib/auth/index";
import type { AuthIdentity } from "@/data/lib/auth/types";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";
import { shortAddress } from "@/data/lib/catalyst/format/address";
import { profileReferralHandle, referralInviteUrl } from "@/data/lib/catalyst/social/referrals";
import { friendProfilesQuery, friendsListQuery } from "@/data/lib/friends/queries";
import { signsFor } from "@/data/lib/profile/queries";

import { useFriendsCount } from "../friends/hooks";
import { useProfileFlags } from "./hooks";
import { profileHref } from "./tabs";

function FriendsList({ identity, close }: { identity: AuthIdentity; close: () => void }) {
  const list = useInfiniteQuery(friendsListQuery(identity));
  const addresses = [...new Set(list.data?.pages.flatMap((page) => page.addresses) ?? [])];
  const profilePages = useQueries({
    queries: list.data?.pages.map((page) => friendProfilesQuery(identity, page.addresses)) ?? [],
  });
  const profiles = Object.assign({}, ...profilePages.map((page) => page.data ?? {})) as NonNullable<
    (typeof profilePages)[number]["data"]
  >;
  return (
    <ProfileFriendsDialog
      friends={
        list.data
          ? addresses.map((address) => ({
              key: address,
              href: profileHref(address),
              name: profiles[address]?.name || shortAddress(address),
              picture: profiles[address]?.picture ?? undefined,
            }))
          : undefined
      }
      pending={list.isPending}
      failed={list.isError && !list.data}
      showMore={list.hasNextPage}
      loadingMore={list.isFetchingNextPage}
      moreFailed={list.isFetchNextPageError}
      close={close}
      onRetry={() => void list.refetch()}
      onMore={() => void list.fetchNextPage()}
      linkComponent={Link}
    />
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
      <ProfileFriendsButton count={count} onOpen={() => setOpen(true)} />
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
  return <ProfileInviteButton href={referralInviteUrl(origin, profileReferralHandle(profile))} />;
}
