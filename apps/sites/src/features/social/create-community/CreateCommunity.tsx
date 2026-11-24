import { useEffect, useState } from "react";
import { useNavigate } from "react-router";

import { CommunityCreate, type CommunityCreateInput } from "@dcl/one-ui/apps/sites";
import { siteUrl } from "@web-core/site";

import type { TrackContext, TrackFn } from "@/core/lib/telemetry/track";

import { submitWith, takeDraft, type CreateFn } from "./submit";

type CreateCommunityProps = {
  trackCtx: TrackContext;
  /** The route's signed create; none while signed out or while the session is restored. */
  create?: CreateFn;
  signedOut: boolean;
  track?: TrackFn;
};

export default function CreateCommunity({ trackCtx, create, signedOut, track }: CreateCommunityProps) {
  const navigate = useNavigate();
  // Read after hydration: the server never has the tab's kept draft.
  const [initial, setInitial] = useState<Partial<CommunityCreateInput>>();
  useEffect(() => {
    const kept = takeDraft();
    if (kept) setInitial(kept);
  }, []);

  return (
    <div className="create-community">
      <CommunityCreate
        key={initial ? "restored" : "fresh"}
        initial={initial}
        create={submitWith(create, { signedOut, trackCtx, track })}
        onDone={(created, result) =>
          navigate(created && result ? `/social/communities/${encodeURIComponent(result.id)}` : "/social")
        }
        contentPolicyUrl={siteUrl("/content")}
        nameClaimUrl="/shop/names"
      />
    </div>
  );
}
