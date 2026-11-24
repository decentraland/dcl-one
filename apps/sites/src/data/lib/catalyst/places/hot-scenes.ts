import { z } from "zod";

/** The node's occupied scenes, in the shape decentraland.org reads from the realm provider. */
export const HOT_SCENES_PATH = "/hot-scenes";

/** A content-server file URL as the node serves it at its own origin; "" for any other value. */
export const ownContentFile = (url: string | null | undefined): string => {
  const hash = /\/contents\/+([A-Za-z0-9]+)$/.exec(url ?? "")?.[1];
  return hash ? `/content/contents/${hash}` : "";
};

const Parcel = z.tuple([z.number(), z.number()]);

/**
 * One scene row a page can render. The node copies `name` from the scene's own title, which a scene can leave out,
 * and a realm provider may still pass an absolute thumbnail URL of the deployer's choice through: a row without a
 * name is not a card, and only a content file is an image.
 */
export const HotScene = z.object({
  id: z.string(),
  name: z.string(),
  baseCoords: Parcel,
  usersTotalCount: z.number(),
  parcels: z.array(Parcel),
  thumbnail: z.string().nullish().transform(ownContentFile),
});
export type HotScene = z.infer<typeof HotScene>;

/** The rows of a `/hot-scenes` answer that fit `row`; a row that does not fit is left out. */
export function hotSceneRows<T>(row: z.ZodType<T>, raw: unknown): T[] {
  return z.array(z.unknown()).parse(raw).flatMap((value) => {
    const result = row.safeParse(value);
    return result.success ? [result.data] : [];
  });
}
