import { PhotoViewer } from "@dcl/one-ui/apps/sites/profile/PhotoViewer";
import { ReelsPage } from "@dcl/one-ui/apps/sites/reels/ReelsPage";

import type { ProfilePhoto } from "@/data/lib/catalyst/social/photos";
import { canReadProfile } from "@/data/lib/profile/queries";
import { reelWearables } from "@/data/lib/reels/queries";

import { photoDestination, photoDetails, photoLabel } from "@/features/photos/photo-view";
import { usePhotoActions } from "@/features/photos/usePhotoActions";

import { CollectibleLink } from "@/features/profile/collectibles";
import { usePlaceOwners } from "@/features/profile/hooks";
import { useReelPlace, useReelWearables } from "@/features/reels/hooks";

export function ReelScreen({ photo, origin }: { photo: ProfilePhoto; origin?: string }) {
  const { position, realm } = photoDestination(photo);
  const addresses = [photo.author.address, ...photo.people.map((person) => person.address)].filter(canReadProfile);
  const faces = usePlaceOwners(addresses);
  const place = useReelPlace(position, realm);
  const worn = useReelWearables(photo.people.flatMap((person) => person.wearables ?? []));
  const details = photoDetails(photo, faces.data ?? {}, place.data ?? undefined, (urns) =>
    urns.length && !worn.isLoading ? reelWearables(urns, worn.data ?? {}) : undefined,
  );
  const actions = usePhotoActions(photo, true, origin);
  const title = photoLabel(photo);

  return (
    <ReelsPage title={title}>
      <PhotoViewer
        variant="page"
        imageUrl={photo.url}
        alt={title}
        label={title}
        details={details}
        actions={actions}
        linkComponent={CollectibleLink}
      />
    </ReelsPage>
  );
}
