import type { PhotoViewerDetails, PhotoViewerPerson } from "@dcl/one-ui";

import { playUrl } from "@/data/lib/catalyst/bases";
import { parsePosition } from "@/data/lib/catalyst/jump/jump";
import { shortAddress } from "@/data/lib/catalyst/format/address";
import { formatPhotoDate, parsePhotoDate, type ProfilePhoto } from "@/data/lib/catalyst/social/photos";
import { canReadProfile } from "@/data/lib/profile/queries";
import { profileHref } from "../profile/tabs";

const WORLD_REALM = /^[a-z0-9-]+\.dcl\.eth$/;

export function reelHref(id: string): string {
  return `/reels/${encodeURIComponent(id.toLowerCase())}`;
}

export function photoLabel(photo: ProfilePhoto): string {
  const place = photo.scene.name || "Decentraland";
  const date = formatPhotoDate(photo.dateTime);
  return date ? `Photo at ${place}, ${date}` : `Photo at ${place}`;
}

export function downloadName(photo: ProfilePhoto, imageType: string): string {
  const name =
    photo.author.name
      .replace(/[^\w-]+/g, "_")
      .replace(/^_+|_+$/g, "")
      .slice(0, 80) || "photo";
  const date = parsePhotoDate(photo.dateTime)?.toISOString().slice(0, 10);
  return `${date ? `${name}-${date}` : name}.${imageType === "image/png" ? "png" : "jpg"}`;
}

function personHref(address: string): string | undefined {
  return canReadProfile(address) ? profileHref(address) : undefined;
}

export function photoDestination(photo: ProfilePhoto) {
  const { x, y } = photo.scene;
  const coordinates = parsePosition(`${x},${y}`);
  const rawRealm = photo.realm.trim().toLowerCase();
  return {
    position: coordinates.isValid ? coordinates.coordinates.join(",") : undefined,
    realm: WORLD_REALM.test(rawRealm) ? rawRealm : undefined,
  };
}

export function photoDetails(
  photo: ProfilePhoto,
  faces: Record<string, { picture?: string | null } | undefined>,
  placeHref?: string,
  wearables?: (urns: string[]) => PhotoViewerPerson["wearables"],
): PhotoViewerDetails {
  const { name } = photo.scene;
  const { position, realm } = photoDestination(photo);
  const jumpInHref = position ? playUrl({ position, realm }) : undefined;
  return {
    date: formatPhotoDate(photo.dateTime) || undefined,
    takenBy: photo.author.name ? { name: photo.author.name, href: personHref(photo.author.address) } : undefined,
    place: { label: position ? `${name} ${position}`.trim() : name, href: placeHref, jumpInHref },
    people: photo.people.map((person) => ({
      name: person.name || shortAddress(person.address),
      href: personHref(person.address),
      faceUrl: faces[person.address]?.picture ?? undefined,
      isGuest: person.isGuest,
      wearables: wearables?.(person.wearables ?? []),
    })),
  };
}
