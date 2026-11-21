import SitesLayout from "@/features/components/chrome/SitesLayout";
import { useEffect, useRef } from "react";

import { pageLoader, type PageDefaults } from "@/core/lib/telemetry/page-loader";
import type { StoryId } from "@/core/lib/telemetry/story-id";
import { track, type TrackContext } from "@/core/lib/telemetry/track";
import { helpConfig } from "@/data/lib/help/config.server";
import { HelpView } from "@/features/help/HelpView";

import { publicBase } from "../lib/social-meta";
import { staticPageMeta } from "../lib/static-page-meta";
import type { Route } from "./+types/help";

const STORY: StoryId = "landings/help";
const FALLBACK: PageDefaults = { variant: "config_driven" };

export function meta({ matches }: Route.MetaArgs) {
  return staticPageMeta("/help", publicBase(matches));
}

export async function loader({ request, context }: Route.LoaderArgs) {
  const { sid, assignment, wrap } = await pageLoader(request, context, FALLBACK);
  return wrap({ ...helpConfig(), sid, variant: assignment.variant });
}

export default function Help({ loaderData }: Route.ComponentProps) {
  const { services, statusPageUrl, intercomAppId, sid, variant } = loaderData;
  const context: TrackContext = { sid, story: STORY, variant };

  const viewed = useRef(false);
  useEffect(() => {
    if (viewed.current) return;
    viewed.current = true;
    track("lp_help_viewed", {}, { sid, story: STORY, variant });
  }, [sid, variant]);

  return (
    // No top-bar entry is highlighted on the help center, as on decentraland.org.
    <SitesLayout active="play">
      <HelpView
        services={services}
        statusPageUrl={statusPageUrl}
        intercomAppId={intercomAppId}
        onCreatorFundClick={(button) => track("lp_help_creator_fund_clicked", { button }, context)}
      />
    </SitesLayout>
  );
}
