import { JumpInvalid, JumpPage } from "@dcl/one-ui";
import SitesLayout from "@/features/components/chrome/SitesLayout";

import { jumpTravelHref } from "@/data/lib/catalyst/jump/jump";

export function JumpInvalidPage({ kind }: { kind: "place" | "event" }) {
  return (
    <SitesLayout active="play" overlayNav>
      <JumpPage invalid>
        <JumpInvalid
          kind={kind}
          jumpInHref={jumpTravelHref({ coordinates: [0, 0], realm: undefined })}
          eventsHref="/events"
          homeHref="/"
        />
      </JumpPage>
    </SitesLayout>
  );
}
