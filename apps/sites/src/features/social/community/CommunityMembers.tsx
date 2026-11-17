import { useInfiniteQuery } from "@tanstack/react-query";
import { useId } from "react";

import { Button, Spinner } from "@dcl/one-ui";
import { GeneralVerified } from "@dcl/one-ui/foundations/icons";

import { useAuth } from "@/data/lib/auth/context";
import { shortAddress } from "@/data/lib/catalyst/format/address";
import type { CommunityInfo, CommunityMember } from "@/data/lib/catalyst/social/community-detail";
import { membersQuery } from "@/data/lib/community/queries";

import styles from "./CommunityLists.module.css";
import { useLoadMore } from "./useLoadMore";
import { faceUrl, roleLabel } from "./view";

/**
 * A private community's members are read signed, as the member viewing it (only members see them);
 * a public one's as anyone, which is what the server prefetched.
 */
function useMembers(community: CommunityInfo) {
  const { identity } = useAuth();
  return useInfiniteQuery(membersQuery(community.id, community.privacy === "private" ? identity : null));
}

export function CommunityMembers({ community }: { community: CommunityInfo }) {
  const titleId = useId();
  const query = useMembers(community);
  const members = query.data?.pages.flatMap((page) => page.members) ?? [];
  const total = query.data?.pages[0]?.total ?? community.membersCount;
  const sentinel = useLoadMore(
    Boolean(query.hasNextPage) && !query.isFetchNextPageError,
    query.isFetchingNextPage, () => void query.fetchNextPage());
  return (
    <section className={styles.section} aria-labelledby={titleId} aria-busy={query.isPending || undefined}>
      <h2 id={titleId} className={styles.title}>
        Members ({total})
      </h2>
      {query.isPending ? (
        <div className={styles.loader}>
          <Spinner variant="light" label="Loading members" />
        </div>
      ) : query.isError && members.length === 0 ? (
        <p className={styles.empty} role="alert">
          We couldn't load the members right now.
        </p>
      ) : members.length === 0 ? (
        <p className={styles.empty}>No members found</p>
      ) : (
        <ul className={styles.members}>
          {members.map((member) => (
            <MemberRow key={member.memberAddress} member={member} />
          ))}
        </ul>
      )}
      {query.isFetchNextPageError ? (
        <div className={styles.loadMoreError} role="alert">
          <p className={styles.empty}>We couldn't load more members.</p>
          <Button variant="secondary" size="sm" onClick={() => void query.fetchNextPage()}>
            Try again
          </Button>
        </div>
      ) : query.hasNextPage ? (
        <div ref={sentinel} className={styles.sentinel}>
          {query.isFetchingNextPage ? <Spinner variant="light" label="Loading more members" /> : null}
        </div>
      ) : null}
    </section>
  );
}

function MemberRow({ member }: { member: CommunityMember }) {
  const name = member.name || shortAddress(member.memberAddress);
  // A path on this site renders the same on the server and in the browser.
  const face = faceUrl(member.profilePictureUrl);
  return (
    <li className={styles.member}>
      {face ? (
        <img className={styles.avatar} src={face} alt="" loading="lazy" />
      ) : (
        <span className={styles.avatar} aria-hidden="true">
          {name.charAt(0).toUpperCase()}
        </span>
      )}
      <span className={styles.memberText}>
        <span className={styles.memberName}>
          <span className={styles.memberLabel}>{name}</span>
          {member.hasClaimedName ? (
            <span className={styles.verified} role="img" aria-label="Claimed name" title="Claimed name">
              <GeneralVerified size={16} aria-hidden="true" />
            </span>
          ) : null}
        </span>
        <span className={styles.role}>{roleLabel(member.role)}</span>
      </span>
    </li>
  );
}
