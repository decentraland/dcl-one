import { useEffect, useState } from "react";
import { Link, useSearchParams } from "react-router";

import { EmptyState } from "@dcl/one-ui/apps/sites/provisional/EmptyState/EmptyState";
import { FilterChip, FilterChipGroup } from "@dcl/one-ui/apps/sites/provisional/FilterChip/FilterChip";
import { PhotoViewer, type PhotoViewerDetails, type PhotoViewerLinkProps } from "@dcl/one-ui/apps/sites/provisional/PhotoViewer/PhotoViewer";
import {
  ProfileCollectibles,
  ProfileCollectiblesItem,
  type ProfileCollectiblesState,
} from "@dcl/one-ui/apps/sites/provisional/profile/ProfileCollectibles";
import { ProfilePhotoTile } from "@dcl/one-ui/apps/sites/provisional/profile/ProfilePhotoTile";
import { JumpInBadgeIcon } from "@dcl/one-ui/apps/sites/provisional/profile/ProfilePlaces";
import { Button } from "@dcl/one-ui/apps/sites/shared/Button/Button";
import { GeneralGallery, GeneralImage, GeneralLock, GeneralPublic } from "@dcl/one-ui/foundations/icons";

import { useAuth } from "@/data/lib/auth/index";
import { playUrl } from "@/data/lib/catalyst/bases";
import { shortAddress } from "@/data/lib/catalyst/format/address";
import { formatPhotoDate, type ProfilePhoto } from "@/data/lib/catalyst/social/photos";
import { normalizeAddress, type ProfileVM } from "@/data/lib/catalyst/social/profile";
import { canReadProfile, signsFor } from "@/data/lib/profile/queries";

import { catalogueTotal, loadedRows, loadMore } from "./collectibles";
import { usePlaceOwners, useProfilePhoto, useProfilePhotos } from "./hooks";
import { ProfileShell, type ProfileShellTab } from "./ProfileShell";
import { profileHref } from "./tabs";

/** The search parameter that holds the open photo, so the viewer can be linked to. */
export const PHOTO_PARAM = "photo";

type Visibility = "all" | "public" | "private";

export type ProfilePhotosScreenProps = {
  profile: ProfileVM;
  own: boolean;
  tabs: ProfileShellTab[];
  onTab: (id: string) => void;
};

function ViewerLink({ to, ...props }: PhotoViewerLinkProps) {
  return <Link to={to} prefetch="intent" {...props} />;
}

function photoCount(n: number): string {
  return n === 1 ? "1 photo" : `${n} photos`;
}

function countLabel(shown: number, total: number): string {
  return total > shown ? `Showing ${shown} of ${photoCount(total)}` : photoCount(shown);
}

/** "Photo at Genesis Plaza, November 08, 2026": what each tile is called. */
function photoLabel(photo: ProfilePhoto): string {
  const date = formatPhotoDate(photo.dateTime);
  return date ? `Photo at ${photo.scene.name}, ${date}` : `Photo at ${photo.scene.name}`;
}

const PARCEL = /^-?\d+$/;

/** Jump In to the parcel it was taken on, in the realm it was taken in (Genesis City for `main`); none without coordinates. */
function jumpInHref(photo: ProfilePhoto): string | undefined {
  const { x, y } = photo.scene;
  if (!PARCEL.test(x.trim()) || !PARCEL.test(y.trim())) return undefined;
  const realm = photo.realm.trim() && photo.realm.trim() !== "main" ? photo.realm.trim() : undefined;
  return playUrl({ position: `${x.trim()},${y.trim()}`, realm });
}

function personHref(address: string): string | undefined {
  return canReadProfile(address) ? profileHref(address) : undefined;
}

function viewerDetails(photo: ProfilePhoto, faces: Record<string, { picture?: string | null } | undefined>): PhotoViewerDetails {
  const takenByHref = personHref(photo.author.address);
  return {
    date: formatPhotoDate(photo.dateTime) || undefined,
    takenBy: photo.author.name && takenByHref ? { name: photo.author.name, href: takenByHref } : undefined,
    place: { label: `${photo.scene.name} ${photo.scene.x},${photo.scene.y}`, jumpInHref: jumpInHref(photo) },
    people: photo.people.map((person) => ({
      name: person.name || shortAddress(person.address),
      href: personHref(person.address),
      faceUrl: faces[person.address]?.picture ?? undefined,
      isGuest: person.isGuest,
    })),
  };
}

/** The open photo with its neighbours in the grid, and the people in it with their faces. */
function OpenPhoto({
  photos,
  index,
  total,
  onOpen,
  onClose,
}: {
  photos: ProfilePhoto[];
  index: number;
  /** How many the position counts against: the gallery's total, or the photos a filter shows. */
  total: number;
  onOpen: (id: string) => void;
  onClose: () => void;
}) {
  const photo = photos[index]!;
  const people = photo.people.flatMap((person) => (canReadProfile(person.address) ? [person.address] : []));
  const faces = usePlaceOwners(people);
  const previous = photos[index - 1];
  const next = photos[index + 1];
  return (
    <PhotoViewer
      imageUrl={photo.url}
      alt={photo.scene.name}
      label={`${photoLabel(photo)}, ${index + 1} of ${Math.max(total, photos.length)}`}
      details={viewerDetails(photo, faces.data ?? {})}
      download={{ href: photo.url, fileName: `${photo.people[0]?.name || "photo"}-${photo.dateTime}.jpg` }}
      onPrevious={previous ? () => onOpen(previous.id) : undefined}
      onNext={next ? () => onOpen(next.id) : undefined}
      onClose={onClose}
      LinkComponent={ViewerLink}
    />
  );
}

/** The profile's Photos tab: a member's snapshots, a page at a time, and a viewer for each. */
export function ProfilePhotosScreen({ profile, own, tabs, onTab }: ProfilePhotosScreenProps) {
  const { identity } = useAuth();
  const ownerIdentity = own ? identity : null;
  const query = useProfilePhotos(profile.address, ownerIdentity);
  // Only the owner's signed list carries private photos, so only it can be split by visibility.
  const signed = signsFor(ownerIdentity, profile.address) && !query.isPlaceholderData;
  const [visibility, setVisibility] = useState<Visibility>("public");
  const [params, setParams] = useSearchParams();
  const openId = params.get(PHOTO_PARAM);

  const rows = query.data ? loadedRows(query.data) : [];
  const filtered = signed && visibility !== "all";
  const shown = filtered ? rows.filter((photo) => photo.isPublic === (visibility === "public")) : rows;

  // The open photo is looked up among the ones shown, so the arrows follow the grid.
  const openIndex = openId === null ? -1 : shown.findIndex((photo) => photo.id === openId);
  const settled = query.data !== undefined && !query.isPlaceholderData;
  // A link to a loaded photo the filter hides (the owner's private one) shows every photo instead.
  const hiddenByFilter = openId !== null && openIndex === -1 && rows.some((photo) => photo.id === openId);
  // A link to a photo past the pages loaded is read by id; it opens only if it is this member's and
  // public, or the owner's own.
  const lookup = useProfilePhoto(settled && openId !== null && openIndex === -1 && !hiddenByFilter ? openId : null);
  const linked =
    lookup.data && lookup.data.author.address === normalizeAddress(profile.address) && (lookup.data.isPublic || signed)
      ? lookup.data
      : null;

  function setOpen(id: string | null) {
    setParams(
      (current) => {
        const next = new URLSearchParams(current);
        if (id === null) next.delete(PHOTO_PARAM);
        else next.set(PHOTO_PARAM, id);
        return next;
      },
      { replace: true, preventScrollReset: true },
    );
  }

  useEffect(() => {
    if (hiddenByFilter) setVisibility("all");
  }, [hiddenByFilter]);

  // A link to a photo that does not exist, or that this member's gallery does not show, opens nothing.
  const stale = settled && openId !== null && openIndex === -1 && !hiddenByFilter && lookup.isSuccess && !linked;
  useEffect(() => {
    if (stale) setOpen(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stale]);

  const filters = signed ? (
    <FilterChipGroup label="Photos shown">
      <FilterChip icon={<GeneralGallery />} label="All" active={visibility === "all"} onSelect={() => setVisibility("all")} />
      <FilterChip
        icon={<GeneralPublic />}
        label="Public"
        active={visibility === "public"}
        onSelect={() => setVisibility("public")}
      />
      <FilterChip
        icon={<GeneralLock />}
        label="Private"
        active={visibility === "private"}
        onSelect={() => setVisibility("private")}
      />
    </FilterChipGroup>
  ) : null;

  let state: ProfileCollectiblesState;
  if (query.isError && !query.data) {
    state = {
      kind: "error",
      message: "Photos are unavailable right now.",
      action: (
        <Button variant="secondary" onClick={() => void query.refetch()}>
          Try again
        </Button>
      ),
    };
  } else if (!query.data) {
    state = { kind: "loading", label: "Loading photos" };
  } else if (rows.length === 0 && !query.hasNextPage) {
    state = own
      ? {
          kind: "empty",
          label: "No photos yet",
          children: (
            <EmptyState
              icon={<GeneralImage />}
              title="No photos yet"
              subtitle="Jump in and start collecting memories! Press 'C' to open the Camera and 'Space bar' to snap a photo."
              action={
                <Button href={playUrl()}>
                  Jump in
                  <JumpInBadgeIcon />
                </Button>
              }
            />
          ),
        }
      : { kind: "empty-note", message: "This member has not taken any photos yet." };
  } else if (shown.length === 0) {
    state = {
      kind: "empty-note",
      message: filtered ? "No photos match this filter." : "None of the photos loaded so far can be shown.",
      more: loadMore(query, "photos"),
    };
  } else {
    state = {
      kind: "list",
      layout: "photos",
      // A filter counts the photos loaded so far; the gallery's total counts every visibility.
      count: filtered ? photoCount(shown.length) : countLabel(shown.length, catalogueTotal(query.data)),
      children: shown.map((photo) => (
        <ProfileCollectiblesItem key={photo.id}>
          <ProfilePhotoTile
            imageUrl={photo.thumbnailUrl}
            label={photoLabel(photo)}
            isPrivate={signed && !photo.isPublic}
            onOpen={() => setOpen(photo.id)}
          />
        </ProfileCollectiblesItem>
      )),
      more: loadMore(query, "photos"),
    };
  }

  return (
    <ProfileShell profile={profile} own={own} tabs={tabs} active="photos" onTab={onTab}>
      <ProfileCollectibles filters={filters} state={state} />
      {openIndex >= 0 ? (
        <OpenPhoto
          photos={shown}
          index={openIndex}
          total={filtered || !query.data ? shown.length : catalogueTotal(query.data)}
          onOpen={setOpen}
          onClose={() => setOpen(null)}
        />
      ) : linked ? (
        <OpenPhoto photos={[linked]} index={0} total={1} onOpen={setOpen} onClose={() => setOpen(null)} />
      ) : null}
    </ProfileShell>
  );
}
