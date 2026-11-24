import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useRouteLoaderData, useSearchParams } from "react-router";

import {
  CollectiblesItem,
  CollectiblesPanel,
  useCategoryTabs,
  useFocusNewRows,
  type CategoryOption,
  type CollectiblesState,
  EmptyMessage,
  PhotoViewer,
  PhotoTile,
  PrivatePhotosNotice,
  GeneralGallery,
  GeneralImage,
  GeneralLock,
  GeneralPublic,
  Button,
  JumpInCTA,
} from "@dcl/one-ui";


import { useAuth } from "@/data/lib/auth/index";
import { playUrl } from "@/data/lib/catalyst/bases";
import { isPhotoId, type ProfilePhoto } from "@/data/lib/catalyst/social/photos";
import { normalizeAddress, type ProfileVM } from "@/data/lib/catalyst/social/profile";
import { canReadProfile, photosQuery, signsFor } from "@/data/lib/profile/queries";

import { downloadName, photoDetails, photoLabel } from "../photos/photo-view";
import { usePhotoActions } from "../photos/usePhotoActions";

import { CollectibleLink, catalogueTotal, loadedRows, loadMore } from "./collectibles";
import { usePlaceOwners, useProfilePhoto, useProfilePhotos } from "./hooks";
import { ProfileShell, type ProfileShellTab } from "./ProfileShell";

/** The search parameter that holds the open photo, so the viewer can be linked to. */
const PHOTO_PARAM = "photo";

type Visibility = "all" | "public" | "private";

const VISIBILITIES: readonly CategoryOption<Visibility>[] = [
  { value: "all", label: "All", icon: <GeneralGallery size={20} aria-hidden="true" /> },
  { value: "public", label: "Public", icon: <GeneralPublic size={20} aria-hidden="true" /> },
  { value: "private", label: "Private", icon: <GeneralLock size={20} aria-hidden="true" /> },
];

export type ProfilePhotosScreenProps = {
  profile: ProfileVM;
  own: boolean;
  tabs: ProfileShellTab[];
  onTab: (id: string) => void;
};

function photoCount(n: number): string {
  return n === 1 ? "1 photo" : `${n} photos`;
}

function countLabel(shown: number, total: number): string {
  return total > shown ? `Showing ${shown} of ${photoCount(total)}` : photoCount(shown);
}

/** The open photo with its neighbours in the grid, and the people in it with their faces. */
function OpenPhoto({
  photos,
  index,
  onOpen,
  onClose,
}: {
  photos: ProfilePhoto[];
  index: number;
  onOpen: (id: string) => void;
  onClose: () => void;
}) {
  const photo = photos[index]!;
  const people = photo.people.flatMap((person) => (canReadProfile(person.address) ? [person.address] : []));
  const faces = usePlaceOwners(people);
  const root = useRouteLoaderData("root") as { publicPageBase?: string } | undefined;
  const actions = usePhotoActions(photo, false, root?.publicPageBase);
  const previous = photos[index - 1];
  const next = photos[index + 1];
  return (
    <PhotoViewer
      imageUrl={photo.url}
      alt={photo.scene.name}
      // Counted against the photos loaded, the ones Previous and Next can reach.
      label={`${photoLabel(photo)}, ${index + 1} of ${photos.length}`}
      details={photoDetails(photo, faces.data ?? {})}
      variant="dialog"
      actions={photo.isPublic ? actions : undefined}
      download={{ href: photo.url, fileName: downloadName(photo, "image/jpeg") }}
      onPrevious={previous ? () => onOpen(previous.id) : undefined}
      onNext={next ? () => onOpen(next.id) : undefined}
      onClose={onClose}
      linkComponent={CollectibleLink}
    />
  );
}

/** The profile's Photos tab: a member's snapshots, a page at a time, and a viewer for each. */
export function ProfilePhotosScreen({ profile, own, tabs, onTab }: ProfilePhotosScreenProps) {
  const { identity } = useAuth();
  const ownerIdentity = own ? identity : null;
  const query = useProfilePhotos(profile.address, ownerIdentity);
  const ownerSigned = signsFor(ownerIdentity, profile.address);
  // A failed first signed read leaves the public list already read on screen, without the filter.
  const client = useQueryClient();
  const fallback =
    ownerSigned && query.isError && !query.data
      ? client.getQueryData(photosQuery(profile.address).queryKey)
      : undefined;
  const data = query.data ?? fallback;
  // Only the owner's signed list carries private photos, so only it can be split by visibility.
  const signed = ownerSigned && !query.isPlaceholderData && !fallback;
  // While the signed list replaces the public one, "nothing to show" is not known yet.
  const awaitingSigned = ownerSigned && query.isPlaceholderData;
  const [visibility, setVisibility] = useState<Visibility>("public");
  const [params, setParams] = useSearchParams();
  const openId = params.get(PHOTO_PARAM);

  const rows = data ? loadedRows(data) : [];
  const filtered = signed && visibility !== "all";
  const shown = filtered ? rows.filter((photo) => photo.isPublic === (visibility === "public")) : rows;
  const focus = useFocusNewRows(shown.length, query.dataUpdatedAt + query.errorUpdatedAt);

  // The open photo is looked up among the ones shown, so the arrows follow the grid.
  const openIndex = openId === null ? -1 : shown.findIndex((photo) => photo.id === openId);
  // The public list kept after a failed signed read counts as settled too.
  const settled = data !== undefined && !query.isPlaceholderData;
  const lookupId = settled && openId !== null && openIndex === -1 && isPhotoId(openId) ? openId : null;
  // A link to a loaded photo the filter hides (the owner's private one) shows every photo instead.
  const hiddenByFilter = openId !== null && openIndex === -1 && rows.some((photo) => photo.id === openId);
  // A link to a photo past the pages loaded is read by id; it opens only if it is this member's and
  // public, or the owner's own.
  const lookup = useProfilePhoto(lookupId !== null && !hiddenByFilter ? lookupId : null);
  const linked =
    lookup.data && lookup.data.author.address === normalizeAddress(profile.address) && (lookup.data.isPublic || signed)
      ? lookup.data
      : null;

  /** Opening from a tile adds a history entry, so Back closes the viewer; stepping and closing replace it. */
  function setOpen(id: string | null, { push = false } = {}) {
    setParams(
      (current) => {
        const next = new URLSearchParams(current);
        if (id === null) next.delete(PHOTO_PARAM);
        else next.set(PHOTO_PARAM, id);
        return next;
      },
      { replace: !push, preventScrollReset: true },
    );
  }

  useEffect(() => {
    if (hiddenByFilter) setVisibility("all");
  }, [hiddenByFilter]);

  // A link to a photo that does not exist, is not a photo id, or that this member's gallery does not
  // show opens nothing.
  const stale =
    settled &&
    openId !== null &&
    openIndex === -1 &&
    !hiddenByFilter &&
    (!isPhotoId(openId) || (lookup.isSuccess && !linked));
  useEffect(() => {
    if (stale) setOpen(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stale]);

  const visibilityTabs = useCategoryTabs("Photos shown", VISIBILITIES, visibility, setVisibility);

  const signedFailed = fallback ? <PrivatePhotosNotice onRetry={() => void query.refetch()} /> : null;

  let state: CollectiblesState;
  if (query.isError && !data) {
    state = {
      kind: "error",
      message: "Photos are unavailable right now.",
      action: (
        <Button
          variant="secondary"
          size="md"
          onClick={() => {
            focus.expectRowsFrom(0);
            void query.refetch();
          }}
        >
          Try again
        </Button>
      ),
    };
  } else if (!data || (rows.length === 0 && awaitingSigned)) {
    state = { kind: "loading", label: "Loading photos" };
  } else if (rows.length === 0 && (fallback || !query.hasNextPage)) {
    state = own
      ? {
          kind: "empty",
          label: "No photos yet",
          children: (
            <EmptyMessage
              icon={<GeneralImage size={56} />}
              title="No photos yet"
              subtitle="Jump in and start collecting memories! Press 'C' to open the Camera and 'Space bar' to snap a photo."
              action={<JumpInCTA href={playUrl()} size="md" />}
            />
          ),
        }
      : { kind: "empty-note", message: "This member has not taken any photos yet." };
  } else if (shown.length === 0) {
    state = {
      kind: "empty-note",
      message: filtered ? "No photos match this filter." : "None of the photos loaded so far can be shown.",
      more: fallback ? undefined : loadMore(query, "photos", () => focus.expectRowsFrom(0)),
    };
  } else {
    state = {
      kind: "list",
      layout: "photos",
      // A filter counts the photos loaded so far; the gallery's total counts every visibility.
      count: filtered ? photoCount(shown.length) : countLabel(shown.length, catalogueTotal(data)),
      children: shown.map((photo) => (
        <CollectiblesItem key={photo.id}>
          <PhotoTile
            image={photo.thumbnailUrl}
            label={photoLabel(photo)}
            isPrivate={signed && !photo.isPublic}
            onOpen={() => setOpen(photo.id, { push: true })}
          />
        </CollectiblesItem>
      )),
      listRef: focus.listRef,
      more: fallback ? undefined : loadMore(query, "photos", () => focus.expectRowsFrom(shown.length)),
    };
  }

  return (
    <ProfileShell profile={profile} own={own} tabs={tabs} active="photos" onTab={onTab}>
      <CollectiblesPanel
        // Only the owner's signed list carries private photos, so only it gets the visibility tabs.
        filters={signed ? visibilityTabs.filters : undefined}
        panel={signed ? visibilityTabs.panel : undefined}
        aside={signedFailed}
        state={state}
        panelRef={focus.panelRef}
      />
      {openIndex >= 0 ? (
        <OpenPhoto photos={shown} index={openIndex} onOpen={(id) => setOpen(id)} onClose={() => setOpen(null)} />
      ) : linked ? (
        <OpenPhoto photos={[linked]} index={0} onOpen={(id) => setOpen(id)} onClose={() => setOpen(null)} />
      ) : null}
    </ProfileShell>
  );
}
