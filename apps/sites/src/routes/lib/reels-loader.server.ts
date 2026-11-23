import { data } from "react-router";

import { fetchPhoto, fetchUserPhotos, isPhotoId } from "@/data/lib/catalyst/social/photos";
import { isEthAddress, normalizeAddress } from "@/data/lib/catalyst/social/profile";

export const reelHeaders = { "Cache-Control": "no-store" };

function notFound(): Response {
  return new Response(null, { status: 404, headers: reelHeaders });
}

function unavailable(error: unknown, signal: AbortSignal): never {
  if (signal.aborted) throw error;
  throw new Response(null, { status: 503, headers: reelHeaders });
}

export async function loadReel(id: string, signal: AbortSignal, read = fetchPhoto) {
  if (!isPhotoId(id)) throw notFound();
  const photo = await read(id.toLowerCase(), { signal }).catch((error: unknown) => unavailable(error, signal));
  if (!photo || photo.id.toLowerCase() !== id.toLowerCase()) throw notFound();
  return data(photo, {
    headers: photo.isPublic ? reelHeaders : { ...reelHeaders, "X-Robots-Tag": "noindex" },
  });
}

export async function loadReelGallery(address: string, signal: AbortSignal, read = fetchUserPhotos) {
  const owner = normalizeAddress(address);
  if (!isEthAddress(owner)) throw notFound();
  // An unknown wallet returns an empty page. A 404 from this Catalyst read signals a failed lookup.
  const page = await read(owner, { limit: 24 }, { signal }).catch((error: unknown) => unavailable(error, signal));
  const photos = page.items.filter((photo) => photo.isPublic && isPhotoId(photo.id));
  if (page.received > 0 && photos.length === 0) unavailable(new Error("Invalid public gallery"), signal);
  return { photos, total: page.total };
}
