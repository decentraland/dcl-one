import { ReelsEmpty, ReelsGallery, ReelsPage } from "@dcl/one-ui/apps/sites/reels/ReelsPage";

import { CollectibleLink } from "@/features/profile/collectibles";
import { useReelThumbnailClick } from "@/features/reels/telemetry";

import { REELS_TITLE, ReelErrorBoundary } from "../lib/reels-route";
import { loadReelGallery, reelHeaders } from "../lib/reels-loader.server";
import { photoLabel, reelHref } from "@/features/photos/photo-view";

import type { Route } from "./+types/reels.list.$address";

export function ErrorBoundary() {
  return <ReelErrorBoundary missing="invalid-gallery" />;
}

export function loader({ params, request }: Route.LoaderArgs) {
  return loadReelGallery(params.address, request.signal);
}

export function headers() {
  return reelHeaders;
}

export function meta() {
  return [{ title: REELS_TITLE }, { name: "robots", content: "noindex" }];
}

export default function ReelsList({ loaderData, params }: Route.ComponentProps) {
  const onPhotoClick = useReelThumbnailClick();
  const { photos: rows, total } = loaderData;
  const name = rows[0]?.author.name || params.address;
  const photos = rows.map((photo) => ({
    id: photo.id,
    href: reelHref(photo.id),
    label: photoLabel(photo),
    thumbnail: photo.thumbnailUrl,
    image: photo.url,
  }));
  return (
    <ReelsPage title={`Reels by ${name}`} gallery>
      {photos.length ? (
        <ReelsGallery photos={photos} total={total} linkComponent={CollectibleLink} onPhotoClick={onPhotoClick} />
      ) : (
        <ReelsEmpty reason="gallery" />
      )}
    </ReelsPage>
  );
}
