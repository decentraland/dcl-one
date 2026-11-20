import type { ReactNode } from "react";

import { Button } from "@dcl/one-ui";
import {
  GeneralLink,
  ProfileCountry,
  ProfileGender,
  ProfileHobbies,
  ProfileLanguage,
  ProfileProfession,
  ProfilePronouns,
  ProfileRealName,
  ProfileRelationship,
} from "@dcl/one-ui/foundations/icons";

import type { ProfileBadge } from "@/data/lib/catalyst/social/badges";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";

import { CollectiblesItem, CollectiblesPanel, CreationCard, type CollectiblesState } from "./collectibles";
import { useProfileBadges, useWornCollectibles } from "./hooks";
import { ProfileBadges } from "./ProfileBadges";
import styles from "./ProfileOverviewScreen.module.css";
import { ProfileShell, type ProfileShellTab } from "./ProfileShell";
import { useBrokenImage } from "./useBrokenImage";

const INFO_ICONS: Record<string, ReactNode> = {
  country: <ProfileCountry size={20} />,
  language: <ProfileLanguage size={20} />,
  pronouns: <ProfilePronouns size={20} />,
  relationship_status: <ProfileRelationship size={20} />,
  gender: <ProfileGender size={20} />,
  profession: <ProfileProfession size={20} />,
  real_name: <ProfileRealName size={20} />,
  favorite_hobby: <ProfileHobbies size={20} />,
};

export type ProfileOverviewScreenProps = {
  profile: ProfileVM;
  /** Whether the wallet has a profile at all; badges are read only then. */
  hasProfile: boolean;
  own: boolean;
  tabs: ProfileShellTab[];
  onTab: (id: string) => void;
};

/** The site is in English; a fixed locale keeps the server and browser renders the same. */
const UNLOCKED = new Intl.DateTimeFormat("en-US", { year: "numeric", month: "short", day: "numeric", timeZone: "UTC" });

export function badgeItems(badges: readonly ProfileBadge[]) {
  return badges.map(({ unlockedAt, ...badge }) => ({
    ...badge,
    unlocked: unlockedAt === undefined ? undefined : `Unlocked on ${UNLOCKED.format(unlockedAt)}`,
  }));
}

/** A second safety net for any caller: only http(s) links are rendered. */
function isHttpUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
}

/** The avatar's full-body snapshot, as the source shows when its 3D preview cannot load. */
function AvatarBody({ imageUrl, name }: { imageUrl?: string; name: string }) {
  const { broken, ref, onError } = useBrokenImage(imageUrl);
  return (
    <div className={styles.body}>
      {imageUrl && !broken ? (
        <img ref={ref} className={styles.bodyImage} src={imageUrl} alt={`${name}'s avatar`} onError={onError} />
      ) : null}
    </div>
  );
}

const NO_WORN_ITEMS = "No equipped items yet.";

/**
 * The collectibles the avatar wears, as the Shop's cards. A visitor gets each card's Buy or View in
 * Shop; on the member's own profile the card only links to the item.
 */
function WornCollectibles({ urns, own }: { urns: readonly string[]; own: boolean }) {
  const query = useWornCollectibles(urns);
  const items = query.data;
  const state: CollectiblesState =
    urns.length === 0
      ? { kind: "empty-note", message: NO_WORN_ITEMS }
      : items === undefined
        ? query.isError
          ? {
              kind: "error",
              message: "Equipped items are unavailable right now.",
              action: (
                <Button variant="secondary" size="md" loading={query.isFetching} onClick={() => void query.refetch()}>
                  Try again
                </Button>
              ),
            }
          : { kind: "loading", label: "Loading equipped items" }
        : items.length === 0
          ? { kind: "empty-note", message: NO_WORN_ITEMS }
          : {
              kind: "list",
              count: items.length === 1 ? "1 equipped item" : `${items.length} equipped items`,
              showCount: false,
              children: items.map((item) => (
                <CollectiblesItem key={item.id}>
                  <CreationCard item={item} actions={!own} />
                </CollectiblesItem>
              )),
            };
  return (
    <div className={styles.section}>
      <h2 className={styles.title}>Equipped items</h2>
      <CollectiblesPanel state={state} />
    </div>
  );
}

/** Overview with the profile site's look: header, tab bar, avatar column, About, Links and the items worn. */
export function ProfileOverviewScreen({
  profile,
  hasProfile,
  own,
  tabs,
  onTab,
}: ProfileOverviewScreenProps) {
  const badges = useProfileBadges(profile.address, hasProfile);
  const bio = profile.bio.trim();
  const emptyBio = own ? "No information yet. Jump in world and complete your bio!" : "This member has not added a bio yet.";
  // The profile read decides each link's shown title; this only refuses a non-http(s) one.
  const links = profile.links.filter((link) => isHttpUrl(link.url));

  return (
    <ProfileShell
      profile={profile}
      own={own}
      tabs={tabs}
      active="overview"
      onTab={onTab}
      aside={<AvatarBody imageUrl={profile.bodyUrl || undefined} name={profile.name} />}
    >
      <div className={styles.root}>
        <section className={styles.surface}>
          <div className={styles.section}>
            <ProfileBadges
              badges={badgeItems(badges.data ?? [])}
              loading={hasProfile && badges.isPending}
              emptyText="No badges yet. Jump in world and start collecting badges!"
              // A failed background re-read keeps the badges already shown.
              errorText={badges.isError && !badges.data ? "Badges are unavailable right now." : undefined}
            />
          </div>
          <div className={styles.section}>
            <h2 className={styles.title}>About</h2>
            {bio ? <p className={styles.bio}>{bio}</p> : <p className={styles.empty}>{emptyBio}</p>}
            {profile.info.length > 0 ? (
              <dl className={styles.info}>
                {profile.info.map((item) => {
                  const icon = INFO_ICONS[item.key];
                  return (
                    <div key={item.key} className={styles.infoItem}>
                      <dt className={styles.infoLabel}>
                        {icon ? <span className={styles.infoIcon}>{icon}</span> : null}
                        {item.label}
                      </dt>
                      <dd className={styles.infoValue}>{item.value}</dd>
                    </div>
                  );
                })}
              </dl>
            ) : null}
          </div>
          {links.length > 0 ? (
            <div className={styles.section}>
              <h2 className={styles.title}>Links</h2>
              <ul className={styles.links}>
                {links.map((link, index) => (
                  <li key={`${index}-${link.url}`}>
                    <a className={styles.link} href={link.url} target="_blank" rel="noopener noreferrer">
                      <GeneralLink size={22} aria-hidden="true" />
                      {link.title}
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
        </section>
        <section className={styles.surface}>
          <WornCollectibles urns={profile.wornCollectibles} own={own} />
        </section>
      </div>
    </ProfileShell>
  );
}
