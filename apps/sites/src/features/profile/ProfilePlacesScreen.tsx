import { useId, useState, type MouseEvent, type ReactNode } from "react";
import { Link } from "react-router";

import {
  Button,
  JumpInCTA,
  PlaceCard,
  Spinner,
  Tabs,
  TabsBar,
  type PlaceCardCreator,
  type PlaceCardLinkProps,
} from "@dcl/one-ui";
import { GeneralFavorites, GeneralMapPin, GeneralPublic, GeneralVerified } from "@dcl/one-ui/foundations/icons";

import { track } from "@/core/lib/telemetry/track";
import { useAuth } from "@/data/lib/auth/index";
import { shortAddress } from "@/data/lib/catalyst/format/address";
import { sceneJumpUrl, worldJumpUrl } from "@/data/lib/catalyst/places/presence";
import { placeLocation, type ProfilePlace } from "@/data/lib/catalyst/places/profile-places";
import { normalizeAddress, type ProfileVM } from "@/data/lib/catalyst/social/profile";
import { openSignIn } from "../components/auth/signin-store";

import { useMyFavoritePlaces, usePlaceOwners, useProfilePlaces } from "./hooks";
import { GET_A_NAME_HREF, MANAGE_WORLD_HREF, ProfileShell, type ProfileShellTab } from "./ProfileShell";
import styles from "./ProfilePlacesScreen.module.css";
import { profileHref } from "./tabs";

/** Where "Explore places" sends a member with no saved place: the Places browse. */
export const EXPLORE_PLACES_HREF = "/places";

type PlacesView = "places" | "favorites";

const OWNER_EMPTY_TITLE = "Nothing here yet, but that's easy to change";

export type ProfilePlacesScreenProps = {
  profile: ProfileVM;
  /** Whether the profile read has settled; until then the owner's NAME is unknown. */
  profileReady: boolean;
  own: boolean;
  tabs: ProfileShellTab[];
  onTab: (id: string) => void;
  sid: string;
};

type PanelState =
  | { kind: "loading" }
  /** A read failed: say so, with the way to try again. Never shown as an empty list. */
  | { kind: "error"; message: string; action: ReactNode }
  /** Nothing to list: the empty message, announced by its title. */
  | { kind: "empty"; title: string; children: ReactNode }
  | { kind: "empty-note"; message: string }
  | { kind: "list"; count: string; children: ReactNode };

function PlaceLink({ to, ...props }: PlaceCardLinkProps) {
  return <Link to={to} prefetch="intent" {...props} />;
}

function jumpHref(place: ProfilePlace, location: string): string {
  return place.world ? worldJumpUrl(location) : sceneJumpUrl(location);
}

/** A ready Jump In link is a completed handoff on the same click, as the place page records it. */
function trackJumpIn(sid: string, placeId: string, launchUrl: string) {
  const context = { sid, story: "jump-in" as const };
  track("jump_in_started", { place_id: placeId }, context);
  track("jump_in_completed", { place_id: placeId, launch_url: launchUrl }, context);
}

/** The count, saying so when the catalogue holds more than the page shows. */
function countLabel(shown: number, total: number): string {
  const noun = (n: number) => (n === 1 ? "1 place" : `${n} places`);
  return total > shown ? `Showing ${shown} of ${noun(total)}` : noun(shown);
}

/** The icon frame, title, text and next step shown when a list has nothing in it. */
export function EmptyMessage({ icon, title, subtitle, action }: { icon: ReactNode; title: string; subtitle: string; action: ReactNode }) {
  return (
    <div className={styles.empty}>
      <span className={styles.emptyIcon} aria-hidden="true">
        {icon}
      </span>
      <div className={styles.emptyBody}>
        <p className={styles.emptyTitle}>{title}</p>
        <p className={styles.emptySubtitle}>{subtitle}</p>
        <div className={styles.emptyAction}>{action}</div>
      </div>
    </div>
  );
}

/** The owner's empty My Places: someone without a NAME needs one, someone with one needs to publish. */
function OwnerEmpty({ hasClaimedName }: { hasClaimedName: boolean }) {
  return hasClaimedName ? (
    <EmptyMessage
      icon={<GeneralMapPin size={56} />}
      title={OWNER_EMPTY_TITLE}
      subtitle="My Places shows the spaces you own or manage in Decentraland. You already have a NAME: publish a World on it and it will appear here."
      action={
        <Button href={MANAGE_WORLD_HREF} size="md" leadingIcon={<GeneralPublic size={20} />}>
          Manage World
        </Button>
      }
    />
  ) : (
    <EmptyMessage
      icon={<GeneralMapPin size={56} />}
      title={OWNER_EMPTY_TITLE}
      subtitle="My Places shows the spaces you own or manage in Decentraland. The easiest way to start is with a NAME, which unlocks your own World."
      action={
        <Button href={GET_A_NAME_HREF} size="md" leadingIcon={<GeneralVerified size={20} />}>
          Get a name
        </Button>
      }
    />
  );
}

/** One live region, always mounted, so a screen reader hears how loading ended. */
function StatusLine({ state }: { state: PanelState }) {
  return (
    <p className={state.kind === "list" ? styles.count : styles.status} role="status">
      {state.kind === "loading" ? (
        <>
          <Spinner variant="light" className={styles.spinner} aria-hidden="true" />
          <span className={styles.visuallyHidden}>Loading places</span>
        </>
      ) : state.kind === "error" ? (
        state.message
      ) : state.kind === "empty-note" ? (
        <span className={styles.note}>{state.message}</span>
      ) : state.kind === "list" ? (
        state.count
      ) : (
        // The empty message below shows this title; the region only announces it.
        <span className={styles.visuallyHidden}>{state.title}</span>
      )}
    </p>
  );
}

/** The profile's Places tab: owned places and worlds, and for the owner also their saved ones. */
export function ProfilePlacesScreen({ profile, profileReady, own, tabs, onTab, sid }: ProfilePlacesScreenProps) {
  const [view, setView] = useState<PlacesView>("places");
  // Saved places belong to the signed caller, so only the owner gets the switch.
  const showFavorites = own && view === "favorites";
  const { identity } = useAuth();
  const ids = useId();

  const owned = useProfilePlaces(profile.address);
  const favorites = useMyFavoritePlaces(own ? identity : null, showFavorites);
  const ownerAddresses = showFavorites
    ? [...new Set((favorites.data?.places ?? []).flatMap((place) => (place.owner ? [place.owner] : [])))]
    : [];
  const owners = usePlaceOwners(ownerAddresses);

  const self: PlaceCardCreator = {
    name: profile.name,
    profileHref: profileHref(profile.address),
    avatar: profile.avatarUrl || undefined,
    avatarBackground: profile.nameColor,
  };

  /** A saved place's owner, once their profile is read; no "by" row until then or without an owner. */
  function ownerOf(place: ProfilePlace): PlaceCardCreator | undefined {
    if (!place.owner) return undefined;
    if (place.owner === normalizeAddress(profile.address)) return self;
    const found = owners.data?.[place.owner];
    if (!found) return undefined;
    return {
      name: found.name || shortAddress(place.owner),
      profileHref: profileHref(place.owner),
      avatar: found.picture ?? undefined,
      avatarBackground: "var(--dcl-neutrals-pale-black)",
    };
  }

  const tabId = (id: PlacesView) => `${ids}-${id}`;
  const panelId = `${ids}-panel`;
  const filters = own ? (
    <TabsBar label="Places shown" variant="pill" className={styles.filters}>
      <Tabs
        id={tabId("places")}
        variant="pill"
        selected={view === "places"}
        aria-controls={panelId}
        onClick={() => setView("places")}
        label={
          <span className={styles.tabLabel}>
            <GeneralMapPin size={20} aria-hidden="true" />
            My Places
          </span>
        }
      />
      <Tabs
        id={tabId("favorites")}
        variant="pill"
        selected={view === "favorites"}
        aria-controls={panelId}
        onClick={() => setView("favorites")}
        label={
          <span className={styles.tabLabel}>
            <GeneralFavorites size={20} aria-hidden="true" />
            Favourites
          </span>
        }
      />
    </TabsBar>
  ) : null;

  const list = showFavorites ? favorites : owned;
  let state: PanelState;
  if (showFavorites && !identity) {
    state = {
      kind: "error",
      message: "Sign in to see your favourites.",
      action: (
        <Button size="md" onClick={() => openSignIn()}>
          Sign in
        </Button>
      ),
    };
  } else if (list.isError && !list.data) {
    state = {
      kind: "error",
      message: showFavorites ? "Your favourites are unavailable right now." : "Places are unavailable right now.",
      action: (
        <Button variant="secondary" size="md" onClick={() => void list.refetch()}>
          Try again
        </Button>
      ),
    };
  } else if (!list.data || (own && !showFavorites && list.data.places.length === 0 && !profileReady)) {
    // The owner's empty state depends on the NAME, so it waits for the profile.
    state = { kind: "loading" };
  } else if (list.data.places.length === 0) {
    state = showFavorites
      ? {
          kind: "empty",
          title: "Nothing saved yet",
          children: (
            <EmptyMessage
              icon={<GeneralFavorites size={56} />}
              title="Nothing saved yet"
              subtitle="Add favorite Places by tapping the heart on a place card or the map in Decentraland. You'll find them all here."
              action={<JumpInCTA href={EXPLORE_PLACES_HREF} size="md" label="Explore places" />}
            />
          ),
        }
      : own
        ? { kind: "empty", title: OWNER_EMPTY_TITLE, children: <OwnerEmpty hasClaimedName={profile.hasClaimedName ?? false} /> }
        : { kind: "empty-note", message: "This member has not registered any places yet." };
  } else {
    const cards: ReactNode[] = list.data.places.map((place) => {
      const location = placeLocation(place);
      const launchUrl = location ? jumpHref(place, location) : undefined;
      // Bridge: PlaceCard's JUMP IN takes no callback yet, so the cell records the click on that link.
      const onJumpIn = launchUrl
        ? (event: MouseEvent<HTMLLIElement>) => {
            const link = (event.target as Element).closest("a");
            if (link?.getAttribute("href") === launchUrl) trackJumpIn(sid, place.id, launchUrl);
          }
        : undefined;
      return (
        <li key={place.id} className={styles.item} onClick={onJumpIn}>
          <PlaceCard
            title={place.title}
            image={place.image}
            href={`/places/${encodeURIComponent(place.id)}`}
            creator={showFavorites ? ownerOf(place) : self}
            coords={location}
            jumpHref={launchUrl}
            Link={PlaceLink}
          />
        </li>
      );
    });
    state = { kind: "list", count: countLabel(list.data.places.length, list.data.total), children: cards };
  }

  const panel = own
    ? { role: "tabpanel", id: panelId, "aria-labelledby": tabId(showFavorites ? "favorites" : "places") }
    : {};

  return (
    <ProfileShell profile={profile} own={own} tabs={tabs} active="places" onTab={onTab}>
      <div className={styles.root}>
        {filters}
        <div {...panel}>
          <StatusLine state={state} />
          {state.kind === "error" ? <div className={styles.errorAction}>{state.action}</div> : null}
          {state.kind === "empty" ? state.children : null}
          {state.kind === "list" ? <ul className={styles.grid}>{state.children}</ul> : null}
        </div>
      </div>
    </ProfileShell>
  );
}
