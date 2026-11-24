// The two pictures the places catalogue gives a scene that has no thumbnail of its own.
const DEFAULT_THUMBNAILS = new Set([
  "bafkreidj26s7aenyxfthfdibnqonzqm5ptc4iamml744gmcyuokewkr76y",
  "bafkreiccptidkodtsbcmk7uw3afmuysu6eboocnr57dmwdg7voylyzg5hy",
]);

/** Whether a place has no picture of its own: none, the map tile, or one of the catalogue's default thumbnails. */
export function isPlaceholderThumbnail(image?: string | null): boolean {
  return !image || image.includes("/v2/map.png") || DEFAULT_THUMBNAILS.has(image.split("/").pop() ?? "");
}

/** The signed-in account's own like (true), dislike (false) or neither (null), and its save. */
export type PlaceViewer = { liked: boolean | null; favorite: boolean };

export function placeViewerOf(row: { user_like: boolean; user_dislike: boolean; user_favorite: boolean }): PlaceViewer {
  return { liked: row.user_like ? true : row.user_dislike ? false : null, favorite: row.user_favorite };
}
