import { z } from "zod";

import type { AuthIdentity } from "../../auth/types";
import { CatalystError, getJSON, signedGetJSON, type GetOptions } from "../client";
import { ImageSchema, MetadataSchema, UserSchema } from "../generated-schemas/camera-reel";
import type { CollectiblePage } from "../marketplace/profile-collectibles";
import { warnInvalid } from "../warn";

/** The node mounts the camera reel here (`services/catalyst/src/serve.rs`). */
const CAMERA_REEL_PREFIX = "/camera-reel-service";

type PhotoPerson = { name: string; address: string; isGuest: boolean };

/** A snapshot as the profile's Photos tab and its viewer show it. */
export type ProfilePhoto = {
  id: string;
  url: string;
  thumbnailUrl: string;
  /** Only the owner's signed listing carries private snapshots; any other listing is public only. */
  isPublic: boolean;
  /** The service's raw value: Unix seconds, or ISO on older rows. */
  dateTime: string;
  realm: string;
  scene: { name: string; x: string; y: string };
  /** Who took it, by the name the service recorded. */
  author: { name: string; address: string };
  people: PhotoPerson[];
};

/** One read of a gallery: its photos, how many rows it sent and how many the gallery holds. */
type PhotoPage = CollectiblePage<ProfilePhoto>;

/**
 * Only the fields the grid and the viewer show are required, so a field the tab does not read
 * (`placeId`, `isEmoting`, a person's wearables) does not drop a photo.
 */
const PhotoRowSchema = ImageSchema.pick({ id: true, url: true, thumbnailUrl: true, isPublic: true }).extend({
  metadata: MetadataSchema.pick({ userName: true, userAddress: true, dateTime: true, realm: true, scene: true }).extend({
    visiblePeople: z.array(UserSchema.pick({ userName: true, userAddress: true, isGuest: true })),
  }),
});

const EnvelopeSchema = z.object({ images: z.array(z.unknown()), currentImages: z.number() });

function httpUrl(value: string): string | undefined {
  const url = value.trim();
  return /^https?:\/\//i.test(url) ? url : undefined;
}

function toPhoto(raw: unknown): ProfilePhoto | null {
  const row = PhotoRowSchema.safeParse(raw);
  const url = row.success ? httpUrl(row.data.url) : undefined;
  if (!row.success || !url) {
    warnInvalid("ProfilePhoto", row.success ? [{ path: ["url"], message: "not an http(s) URL" }] : row.error.issues);
    return null;
  }
  const { id, thumbnailUrl, isPublic, metadata } = row.data;
  return {
    id,
    url,
    thumbnailUrl: httpUrl(thumbnailUrl) ?? url,
    isPublic,
    dateTime: metadata.dateTime,
    realm: metadata.realm,
    scene: { name: metadata.scene.name, x: metadata.scene.location.x, y: metadata.scene.location.y },
    author: { name: metadata.userName.trim(), address: metadata.userAddress.toLowerCase() },
    people: metadata.visiblePeople.map((person) => ({
      name: person.userName.trim(),
      address: person.userAddress.toLowerCase(),
      isGuest: person.isGuest,
    })),
  };
}

/**
 * A wallet's snapshots, newest first. The service lists private snapshots only to a request signed
 * by that wallet, so `identity` is passed for the owner's own gallery and never for anyone else's.
 */
export async function fetchUserPhotos(
  address: string,
  params: { limit: number; offset?: number },
  opts: Pick<GetOptions, "signal" | "base"> & { identity?: AuthIdentity } = {},
): Promise<PhotoPage> {
  const path = `${CAMERA_REEL_PREFIX}/api/users/${encodeURIComponent(address.trim().toLowerCase())}/images`;
  const query = { limit: params.limit, offset: params.offset ?? 0 };
  const raw = opts.identity
    ? await signedGetJSON<unknown>(path, { identity: opts.identity, base: opts.base, signal: opts.signal, query })
    : await getJSON<unknown>(path, { base: opts.base, signal: opts.signal, query });
  const page = EnvelopeSchema.safeParse(raw);
  if (!page.success) throw new CatalystError("the response did not have the gallery shape", path);
  const items: ProfilePhoto[] = [];
  for (const image of page.data.images) {
    const photo = toPhoto(image);
    if (photo) items.push(photo);
  }
  return { items, received: page.data.images.length, total: page.data.currentImages };
}

/** The service's image ids are UUIDs; anything else is not a photo and is never requested. */
export function isPhotoId(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value);
}

/**
 * One snapshot by id, for a link to a photo past the pages loaded. `null` when the id is not one,
 * the service does not have it (or withholds it), or the row cannot be shown.
 */
export async function fetchPhoto(id: string, opts: Pick<GetOptions, "signal" | "base"> = {}): Promise<ProfilePhoto | null> {
  if (!isPhotoId(id)) return null;
  const path = `${CAMERA_REEL_PREFIX}/api/images/${id.toLowerCase()}/metadata`;
  try {
    return toPhoto(await getJSON<unknown>(path, opts));
  } catch (error) {
    if (error instanceof CatalystError && error.status === 404) return null;
    throw error;
  }
}

/**
 * The date the viewer prints, as the profile site does ("November 08, 2026"); empty when unreadable.
 * In UTC, so the server's render and the browser's agree.
 */
export function formatPhotoDate(dateTime: string): string {
  const value = dateTime.trim();
  const numeric = Number(value);
  const date = value !== "" && Number.isFinite(numeric) ? new Date(numeric * 1000) : new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleDateString("en-US", { month: "long", day: "2-digit", year: "numeric", timeZone: "UTC" });
}
