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
import { useReelEvents } from "@/features/reels/telemetry";

export function ReelScreen({ photo, origin }: { photo: ProfilePhoto; origin?: string }) {
  const { position, realm } = photoDestination(photo);
  const addresses = [photo.author.address, ...photo.people.map((person) => person.address)].filter(canReadProfile);
  const faces = usePlaceOwners(addresses);
  const place = useReelPlace(position, realm);
  const worn = useReelWearables(photo.people.flatMap((person) => person.wearables ?? []));
  const details = photoDetails(photo, faces.data ?? {}, place.data ?? undefined, (urns) =>
    urns.length && !worn.isLoading ? reelWearables(urns, worn.data ?? {}) : undefined,
  );
  // The list a person's wearables showed, read again at a click for the URN it was worn as.
  const events = useReelEvents(photo, (person, wearable) =>
    worn.data ? reelWearables(photo.people[person]?.wearables ?? [], worn.data)[wearable]?.urn : undefined,
  );
  const shown = usePhotoActions(photo, true, origin, { onCopied: events.onCopied });
  const save = shown.onDownload;
  // decentraland.org counted a download at the press, before the image was fetched.
  const actions = save
    ? {
        ...shown,
        onDownload: () => {
          events.onDownload();
          save();
        },
      }
    : shown;
  const title = photoLabel(photo);

  return (
    <ReelsPage title={title} onHomeClick={events.onHomeClick}>
      <PhotoViewer
        variant="page"
        imageUrl={photo.url}
        alt={title}
        label={title}
        details={details}
        actions={actions}
        events={events.viewer}
        linkComponent={CollectibleLink}
      />
    </ReelsPage>
  );
}
