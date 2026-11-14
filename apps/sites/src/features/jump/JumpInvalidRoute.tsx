import { JumpInvalid } from "@dcl/one-ui/apps/sites/jump/JumpInvalid";
import { JumpPage } from "@dcl/one-ui/apps/sites/jump/JumpPage";
import SitesChrome from "@ui/web/frames/SitesChrome";

import { jumpTravelHref } from "@/data/lib/catalyst/jump/jump";

export function JumpInvalidRoute({ kind }: { kind: "place" | "event" }) {
  return (
    <SitesChrome active="play">
      <JumpPage>
        <JumpInvalid
          kind={kind}
          jumpInHref={jumpTravelHref({ coordinates: [0, 0], realm: undefined })}
          eventsHref="/events"
          homeHref="/"
        />
      </JumpPage>
    </SitesChrome>
  );
}
