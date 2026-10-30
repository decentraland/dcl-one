import { getJSON } from "../client";
import type { GetOptions } from "../client";
import {
  GetImagesResponseSchema,
  ImageSchema,
} from "../generated-schemas/camera-reel";
import { z } from "zod";

type WireImage = z.infer<typeof ImageSchema>;
export type GalleryImage = WireImage & { dateTime: string };

function liftDateTime(img: WireImage): GalleryImage {
  return { ...img, dateTime: img.metadata.dateTime };
}

export async function fetchUserPhotos(
  address: string,
  opts: GetOptions = {},
): Promise<GalleryImage[]> {
  const raw = await getJSON<unknown>(
    `/camera-reel/api/users/${encodeURIComponent(address.toLowerCase())}/images`,
    opts,
  );
  return GetImagesResponseSchema.parse(raw).images.map(liftDateTime);
}
