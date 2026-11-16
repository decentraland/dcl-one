import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useId, useState, type ReactNode, type Ref } from "react";
import { useSearchParams } from "react-router";

import { Button, JumpInCTA, Spinner, Tabs, TabsBar } from "@dcl/one-ui";
import { GeneralGallery, GeneralImage, GeneralLock, GeneralPublic } from "@dcl/one-ui/foundations/icons";

import { useAuth } from "@/data/lib/auth/index";
import { playUrl } from "@/data/lib/catalyst/bases";
import { shortAddress } from "@/data/lib/catalyst/format/address";
import { formatPhotoDate, isPhotoId, type ProfilePhoto } from "@/data/lib/catalyst/social/photos";
import { normalizeAddress, type ProfileVM } from "@/data/lib/catalyst/social/profile";
import { canReadProfile, photosQuery, signsFor } from "@/data/lib/profile/queries";

import { catalogueTotal, loadedRows, loadMore, useFocusNewRows } from "./collectibles";
import { usePlaceOwners, useProfilePhoto, useProfilePhotos } from "./hooks";
import { PhotoViewer, type PhotoViewerDetails } from "./PhotoViewer";
import { ProfileShell, type ProfileShellTab } from "./ProfileShell";
import styles from "./ProfilePhotosScreen.module.css";
import { profileHref } from "./tabs";

/** The search parameter that holds the open photo, so the viewer can be linked to. */
const PHOTO_PARAM = "photo";

type Visibility = "all" | "public" | "private";

const VISIBILITIES: { id: Visibility; label: string; icon: ReactNode }[] = [
  { id: "all", label: "All", icon: <GeneralGallery size={20} aria-hidden="true" /> },
  { id: "public", label: "Public", icon: <GeneralPublic size={20} aria-hidden="true" /> },
  { id: "private", label: "Private", icon: <GeneralLock size={20} aria-hidden="true" /> },
];

type PanelState =
  | { kind: "loading" }
  /** A read failed: say so, with the way to try again. Never shown as an empty list. */
  | { kind: "error"; message: string; action: ReactNode }
  /** Nothing to list: the empty message, announced by its title. */
  | { kind: "empty"; title: string; children: ReactNode }
  /** `more` keeps Load more under a note when a filter matched nothing on the pages loaded so far. */
  | { kind: "empty-note"; message: string; more?: ReactNode }
  | { kind: "list"; count: string; children: ReactNode; listRef: Ref<HTMLUListElement>; more?: ReactNode };

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

/** "Photo at Genesis Plaza, November 08, 2026": what each tile is called. */
function photoLabel(photo: ProfilePhoto): string {
  const date = formatPhotoDate(photo.dateTime);
  return date ? `Photo at ${photo.scene.name}, ${date}` : `Photo at ${photo.scene.name}`;
}

const PARCEL = /^-?\d+$/;

/** A world NAME, the only realm Jump In forwards: the photo's realm is whatever its uploader wrote. */
const WORLD_REALM = /^[a-z0-9-]+\.dcl\.eth$/;

/**
 * Jump In to the parcel it was taken on, in its world when the realm is one; Genesis City for any
 * other realm, since an uploaded realm URL could send the Explorer to a server of the uploader's
 * choosing. None without coordinates.
 */
function jumpInHref(photo: ProfilePhoto): string | undefined {
  const { x, y } = photo.scene;
  if (!PARCEL.test(x.trim()) || !PARCEL.test(y.trim())) return undefined;
  const raw = photo.realm.trim().toLowerCase();
  return playUrl({ position: `${x.trim()},${y.trim()}`, realm: WORLD_REALM.test(raw) ? raw : undefined });
}

/** "kurd-2026-11-10.jpg": who took it and the day, as a file name. */
function downloadName(photo: ProfilePhoto): string {
  const name = photo.author.name.replace(/[^\w.-]+/g, "_").replace(/^_+|_+$/g, "") || "photo";
  const seconds = Number(photo.dateTime.trim());
  const date = new Date(Number.isFinite(seconds) && photo.dateTime.trim() !== "" ? seconds * 1000 : photo.dateTime);
  return Number.isNaN(date.getTime()) ? `${name}.jpg` : `${name}-${date.toISOString().slice(0, 10)}.jpg`;
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

/** A snapshot in the grid; the whole tile opens the viewer. */
function PhotoTile({ photo, label, isPrivate, onOpen }: { photo: ProfilePhoto; label: string; isPrivate: boolean; onOpen: () => void }) {
  // The failed URL, so a new thumbnail for the same tile is tried again.
  const [brokenUrl, setBrokenUrl] = useState<string | null>(null);
  const broken = brokenUrl === photo.thumbnailUrl;
  return (
    <button type="button" className={styles.tile} aria-label={isPrivate ? `${label}, private` : label} onClick={onOpen}>
      {broken ? (
        <span className={styles.tileBroken} aria-hidden="true">
          <GeneralImage size={40} />
        </span>
      ) : (
        <img
          className={styles.tileImage}
          src={photo.thumbnailUrl}
          alt=""
          loading="lazy"
          onError={() => setBrokenUrl(photo.thumbnailUrl)}
        />
      )}
      {isPrivate ? (
        <span className={styles.privateBadge} aria-hidden="true">
          <GeneralLock size={16} />
        </span>
      ) : null}
    </button>
  );
}

/** The owner's empty gallery: the icon frame, title, text and the way in-world. */
function OwnerEmpty() {
  return (
    <div className={styles.empty}>
      <span className={styles.emptyIcon} aria-hidden="true">
        <GeneralImage size={56} />
      </span>
      <div className={styles.emptyBody}>
        <p className={styles.emptyTitle}>No photos yet</p>
        <p className={styles.emptySubtitle}>
          Jump in and start collecting memories! Press &apos;C&apos; to open the Camera and &apos;Space bar&apos; to snap a photo.
        </p>
        <div className={styles.emptyAction}>
          <JumpInCTA href={playUrl()} size="md" />
        </div>
      </div>
    </div>
  );
}

/** One live region, always mounted, so a screen reader hears how loading ended. */
function StatusLine({ state }: { state: PanelState }) {
  return (
    <p className={state.kind === "list" ? styles.count : styles.status} role="status">
      {state.kind === "loading" ? (
        <>
          <Spinner variant="light" className={styles.spinner} aria-hidden="true" />
          <span className={styles.visuallyHidden}>Loading photos</span>
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
  const previous = photos[index - 1];
  const next = photos[index + 1];
  return (
    <PhotoViewer
      imageUrl={photo.url}
      alt={photo.scene.name}
      // Counted against the photos loaded, the ones Previous and Next can reach.
      label={`${photoLabel(photo)}, ${index + 1} of ${photos.length}`}
      details={viewerDetails(photo, faces.data ?? {})}
      download={{ href: photo.url, fileName: downloadName(photo) }}
      onPrevious={previous ? () => onOpen(previous.id) : undefined}
      onNext={next ? () => onOpen(next.id) : undefined}
      onClose={onClose}
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
  const settled = query.data !== undefined && !query.isPlaceholderData;
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

  const ids = useId();
  const tabId = (id: Visibility) => `${ids}-${id}`;
  const panelId = `${ids}-panel`;
  const filters = signed ? (
    <TabsBar label="Photos shown" variant="pill" className={styles.filters}>
      {VISIBILITIES.map(({ id, label, icon }) => (
        <Tabs
          key={id}
          id={tabId(id)}
          variant="pill"
          selected={visibility === id}
          aria-controls={panelId}
          onClick={() => setVisibility(id)}
          label={
            <span className={styles.tabLabel}>
              {icon}
              {label}
            </span>
          }
        />
      ))}
    </TabsBar>
  ) : null;

  const signedFailed = fallback ? (
    <span role="status">
      Your private photos could not be loaded.{" "}
      <Button variant="text" size="sm" onClick={() => void query.refetch()}>
        Try again
      </Button>
    </span>
  ) : null;

  let state: PanelState;
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
    state = { kind: "loading" };
  } else if (rows.length === 0 && (fallback || !query.hasNextPage)) {
    state = own
      ? { kind: "empty", title: "No photos yet", children: <OwnerEmpty /> }
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
      // A filter counts the photos loaded so far; the gallery's total counts every visibility.
      count: filtered ? photoCount(shown.length) : countLabel(shown.length, catalogueTotal(data)),
      children: shown.map((photo) => (
        <li key={photo.id} className={styles.item}>
          <PhotoTile
            photo={photo}
            label={photoLabel(photo)}
            isPrivate={signed && !photo.isPublic}
            onOpen={() => setOpen(photo.id, { push: true })}
          />
        </li>
      )),
      listRef: focus.listRef,
      more: fallback ? undefined : loadMore(query, "photos", () => focus.expectRowsFrom(shown.length)),
    };
  }

  const more = state.kind === "list" || state.kind === "empty-note" ? state.more : undefined;
  const panel = signed
    ? { role: "tabpanel", id: panelId, "aria-labelledby": tabId(visibility) }
    : {};

  return (
    <ProfileShell profile={profile} own={own} tabs={tabs} active="photos" onTab={onTab}>
      <div ref={focus.panelRef} className={styles.root} tabIndex={-1}>
        {filters || signedFailed ? (
          <div className={styles.header}>
            {filters}
            {signedFailed ? <div className={styles.aside}>{signedFailed}</div> : null}
          </div>
        ) : null}
        <div {...panel}>
          <StatusLine state={state} />
          {state.kind === "error" ? <div className={styles.errorAction}>{state.action}</div> : null}
          {state.kind === "empty" ? state.children : null}
          {state.kind === "list" ? (
            <ul ref={state.listRef} className={styles.grid}>
              {state.children}
            </ul>
          ) : null}
          {more ? <div className={styles.more}>{more}</div> : null}
        </div>
      </div>
      {openIndex >= 0 ? (
        <OpenPhoto photos={shown} index={openIndex} onOpen={(id) => setOpen(id)} onClose={() => setOpen(null)} />
      ) : linked ? (
        <OpenPhoto photos={[linked]} index={0} onOpen={(id) => setOpen(id)} onClose={() => setOpen(null)} />
      ) : null}
    </ProfileShell>
  );
}
