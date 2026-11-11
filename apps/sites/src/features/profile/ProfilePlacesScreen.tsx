import { useState, type ReactNode } from "react";
import { Link } from "react-router";

import { EmptyState } from "@dcl/one-ui/apps/sites/provisional/EmptyState/EmptyState";
import { FilterChip, FilterChipGroup } from "@dcl/one-ui/apps/sites/provisional/FilterChip/FilterChip";
import {
  JumpInBadgeIcon,
  ProfilePlaces,
  ProfilePlacesItem,
  type ProfilePlacesState,
} from "@dcl/one-ui/apps/sites/provisional/profile/ProfilePlaces";
import {
  SceneCard,
  type SceneCardCreator,
  type SceneCardLinkProps,
} from "@dcl/one-ui/apps/sites/provisional/SceneCard/SceneCard";
import { Button } from "@dcl/one-ui/apps/sites/shared/Button/Button";
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
import { profileHref } from "./tabs";

/** Where "Explore places" sends a member with no saved place: the Places browse. */
export const EXPLORE_PLACES_HREF = "/places";

type PlacesView = "places" | "favorites";

export type ProfilePlacesScreenProps = {
  profile: ProfileVM;
  /** Whether the profile read has settled; until then the owner's NAME is unknown. */
  profileReady: boolean;
  own: boolean;
  tabs: ProfileShellTab[];
  onTab: (id: string) => void;
  sid: string;
};

function PlaceLink({ to, ...props }: SceneCardLinkProps) {
  return <Link to={to} prefetch="intent" {...props} />;
}

function jumpHref(place: ProfilePlace): string {
  return place.world && place.worldName ? worldJumpUrl(place.worldName) : sceneJumpUrl(place.basePosition);
}

/** A ready Jump In link is a completed handoff on the same click, as the place page records it. */
function trackJumpIn(sid: string, placeId: string, launchUrl: string) {
  const context = { sid, story: "jump-in" as const };
  track("jump_in_started", { place_id: placeId }, context);
  track("jump_in_completed", { place_id: placeId, launch_url: launchUrl }, context);
}

function countLabel(count: number): string {
  return count === 1 ? "1 place" : `${count} places`;
}

/** The owner's empty My Places: someone without a NAME needs one, someone with one needs to publish. */
function OwnerEmpty({ hasClaimedName }: { hasClaimedName: boolean }) {
  return hasClaimedName ? (
    <EmptyState
      icon={<GeneralMapPin />}
      title="Nothing here yet, but that's easy to change"
      subtitle="My Places shows the spaces you own or manage in Decentraland. You already have a NAME: publish a World on it and it will appear here."
      action={
        <Button href={MANAGE_WORLD_HREF}>
          <GeneralPublic size={20} aria-hidden="true" />
          Manage World
        </Button>
      }
    />
  ) : (
    <EmptyState
      icon={<GeneralMapPin />}
      title="Nothing here yet, but that's easy to change"
      subtitle="My Places shows the spaces you own or manage in Decentraland. The easiest way to start is with a NAME, which unlocks your own World."
      action={
        <Button href={GET_A_NAME_HREF}>
          <GeneralVerified size={20} aria-hidden="true" />
          Get a name
        </Button>
      }
    />
  );
}

/** The profile's Places tab: owned places and worlds, and for the owner also their saved ones. */
export function ProfilePlacesScreen({ profile, profileReady, own, tabs, onTab, sid }: ProfilePlacesScreenProps) {
  const [view, setView] = useState<PlacesView>("places");
  // Saved places belong to the signed caller, so only the owner gets the switch.
  const showFavorites = own && view === "favorites";
  const { identity } = useAuth();

  const owned = useProfilePlaces(profile.address);
  const favorites = useMyFavoritePlaces(own ? identity : null, showFavorites);
  const ownerAddresses = showFavorites
    ? [...new Set((favorites.data ?? []).flatMap((place) => (place.owner ? [place.owner] : [])))]
    : [];
  const owners = usePlaceOwners(ownerAddresses);

  const self: SceneCardCreator = {
    name: profile.name,
    href: profileHref(profile.address),
    faceUrl: profile.avatarUrl || undefined,
    color: profile.nameColor,
  };

  /** A saved place's owner, once their profile is read; no "by" row until then or without an owner. */
  function ownerOf(place: ProfilePlace): SceneCardCreator | undefined {
    if (!place.owner) return undefined;
    if (place.owner === normalizeAddress(profile.address)) return self;
    const found = owners.data?.[place.owner];
    if (!found) return undefined;
    return {
      name: found.name || shortAddress(place.owner),
      href: profileHref(place.owner),
      faceUrl: found.picture ?? undefined,
      color: "var(--dcl-neutrals-pale-black)",
    };
  }

  const filters = own ? (
    <FilterChipGroup label="Places shown">
      <FilterChip
        icon={<GeneralMapPin />}
        label="My Places"
        active={view === "places"}
        onSelect={() => setView("places")}
      />
      <FilterChip
        icon={<GeneralFavorites />}
        label="Favourites"
        active={view === "favorites"}
        onSelect={() => setView("favorites")}
      />
    </FilterChipGroup>
  ) : null;

  const list = showFavorites ? favorites : owned;
  let state: ProfilePlacesState;
  if (showFavorites && !identity) {
    state = {
      kind: "error",
      message: "Sign in to see your favourites.",
      action: <Button onClick={() => openSignIn()}>Sign in</Button>,
    };
  } else if (list.isError && !list.data) {
    state = {
      kind: "error",
      message: showFavorites
        ? "Your favourites are unavailable right now."
        : "Places are unavailable right now.",
      action: (
        <Button variant="secondary" onClick={() => void list.refetch()}>
          Try again
        </Button>
      ),
    };
  } else if (!list.data || (own && !showFavorites && list.data.length === 0 && !profileReady)) {
    // The owner's empty state depends on the NAME, so it waits for the profile.
    state = { kind: "loading", label: "Loading places" };
  } else if (list.data.length === 0) {
    state = showFavorites
      ? {
          kind: "empty",
          children: (
            <EmptyState
              icon={<GeneralFavorites />}
              title="Nothing saved yet"
              subtitle="Add favorite Places by tapping the heart on a place card or the map in Decentraland. You'll find them all here."
              action={
                <Button href={EXPLORE_PLACES_HREF}>
                  Explore places
                  <JumpInBadgeIcon />
                </Button>
              }
            />
          ),
        }
      : own
        ? { kind: "empty", children: <OwnerEmpty hasClaimedName={profile.hasClaimedName ?? false} /> }
        : { kind: "empty-note", message: "This member has not registered any places yet." };
  } else {
    const cards: ReactNode[] = list.data.map((place) => {
      const href = jumpHref(place);
      return (
        <ProfilePlacesItem key={place.id}>
          <SceneCard
            title={place.title}
            imageUrl={place.image}
            href={`/places/${encodeURIComponent(place.id)}`}
            creator={showFavorites ? ownerOf(place) : self}
            location={placeLocation(place)}
            jumpIn={{ href, onJumpIn: () => trackJumpIn(sid, place.id, href) }}
            LinkComponent={PlaceLink}
          />
        </ProfilePlacesItem>
      );
    });
    state = { kind: "list", count: countLabel(list.data.length), children: cards };
  }

  return (
    <ProfileShell profile={profile} own={own} tabs={tabs} active="places" onTab={onTab}>
      <ProfilePlaces filters={filters} state={state} />
    </ProfileShell>
  );
}
