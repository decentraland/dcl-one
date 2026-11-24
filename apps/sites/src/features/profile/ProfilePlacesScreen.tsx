import { useState } from "react";
import { Link } from "react-router";

import {
  GeneralFavorites,
  Button,
  JumpInCTA,
  type PlaceCardCreator,
  type PlaceCardLinkProps,
} from "@dcl/one-ui";
import {
  OWNER_EMPTY_TITLE,
  PlacesOwnerEmpty,
  ProfilePlaces,
  type PlacesPanelState,
  EmptyMessage,
} from "@dcl/one-ui/apps/sites";


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

/** The profile's Places tab: owned places and worlds, and for the owner also their saved ones. */
export function ProfilePlacesScreen({ profile, profileReady, own, tabs, onTab, sid }: ProfilePlacesScreenProps) {
  const [view, setView] = useState<PlacesView>("places");
  // Saved places belong to the signed caller, so only the owner gets the switch.
  const showFavorites = own && view === "favorites";
  const { identity } = useAuth();

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

  const list = showFavorites ? favorites : owned;
  let state: PlacesPanelState;
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
        ? {
            kind: "empty",
            title: OWNER_EMPTY_TITLE,
            children: (
              <PlacesOwnerEmpty
                hasClaimedName={profile.hasClaimedName ?? false}
                manageWorldHref={MANAGE_WORLD_HREF}
                getNameHref={GET_A_NAME_HREF}
              />
            ),
          }
        : { kind: "empty-note", message: "This member has not registered any places yet." };
  } else {
    const cards = list.data.places.map((place) => {
      const location = placeLocation(place);
      const launchUrl = location ? jumpHref(place, location) : undefined;
      return {
        id: place.id,
        title: place.title,
        image: place.image,
        href: `/places/${encodeURIComponent(place.id)}`,
        creator: showFavorites ? ownerOf(place) : self,
        coords: location,
        jumpHref: launchUrl,
        Link: PlaceLink,
        onJumpIn: launchUrl ? () => trackJumpIn(sid, place.id, launchUrl) : undefined,
      };
    });
    state = { kind: "list", count: countLabel(list.data.places.length, list.data.total), cards };
  }

  return (
    <ProfileShell profile={profile} own={own} tabs={tabs} active="places" onTab={onTab}>
      <ProfilePlaces own={own} view={view} onView={setView} state={state} />
    </ProfileShell>
  );
}
