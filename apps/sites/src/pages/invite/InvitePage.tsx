import { lazy, Suspense, useEffect, useState } from "react";
import { InviteLanding } from "@dcl/one-ui";
import { docsUrl } from "@web-core/docs";

import SitesLayout from "@/features/components/chrome/SitesLayout";

import { referralUrl } from "./referral-url";

// @dcl/one-ui has no avatar renderer for the web: the page draws the inviter's avatar into the landing's slot.
const WearablePreview = lazy(() => import("@ui/wearable-preview/WearablePreview"));

type InvitePageProps = {
  referrerName: string | null;
  referrerAddress: string | null;
};

/** The invite landing without the site navigation, as decentraland.org shows it. */
export function InvitePage({ referrerName, referrerAddress }: InvitePageProps) {
  // The renderer runs in the browser only: the server and the first client render leave the slot empty.
  const [client, setClient] = useState(false);
  useEffect(() => setClient(true), []);
  const address = referrerAddress?.toLowerCase() ?? "";
  return (
    <SitesLayout hideNav>
      <InviteLanding
        referrerName={referrerName}
        joinHref={referralUrl(address)}
        faqsHref={docsUrl("player/faqs")}
        avatar={
          client && address ? (
            <Suspense fallback={null}>
              <WearablePreview profile={address} controls={false} />
            </Suspense>
          ) : null
        }
      />
    </SitesLayout>
  );
}
