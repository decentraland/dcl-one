import { JumpInvalid } from "@dcl/one-ui/apps/sites/jump/JumpInvalid";
import { JumpPage } from "@dcl/one-ui/apps/sites/jump/JumpPage";
import SitesChrome from "@ui/web/frames/SitesChrome";

import { jumpTravelHref } from "@/data/lib/catalyst/jump/jump";

export function JumpInvalidRoute({ kind }: { kind: "place" | "event" }) {
  return (
    <SitesChrome active="play" overlayNav>
      <JumpPage invalid>
        <JumpInvalid
          kind={kind}
          jumpInHref={jumpTravelHref({ coordinates: [0, 0], realm: undefined })}
          eventsHref="/whats-on"
          homeHref="/"
        />
      </JumpPage>
    </SitesChrome>
  );
}
