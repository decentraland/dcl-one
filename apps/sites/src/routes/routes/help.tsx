import "@dcl/one-ui/foundations/foundations.css";
import SitesChrome from "@ui/web/frames/SitesChrome";

import { pageLoader, type PageDefaults } from "@/core/lib/telemetry/page-loader";
import type { StoryId } from "@/core/lib/telemetry/story-id";
import { track } from "@/core/lib/telemetry/track";
import { helpConfig } from "@/data/lib/help/config.server";
import { HELP_COPY } from "@/features/help/content";
import { HelpView } from "@/features/help/HelpView";

import type { Route } from "./+types/help";

const STORY: StoryId = "landings/help";
const FALLBACK: PageDefaults = { variant: "config_driven" };

export function meta() {
  return [
    { title: HELP_COPY.pageTitle },
    { name: "description", content: HELP_COPY.description },
  ];
}

export async function loader({ request }: Route.LoaderArgs) {
  const { sid, wallet, wrap } = await pageLoader(request, FALLBACK);
  return wrap({ ...helpConfig(), sid, wallet });
}

export default function Help({ loaderData }: Route.ComponentProps) {
  const { services, statusPageUrl, intercomAppId, sid, wallet } = loaderData;
  return (
    // No top-bar entry is highlighted on the help center, as on decentraland.org.
    <SitesChrome active="play">
      <HelpView
        services={services}
        statusPageUrl={statusPageUrl}
        intercomAppId={intercomAppId}
        wallet={wallet}
        onCreatorFundClick={(button) =>
          track("lp_help_creator_fund_clicked", { button }, { sid, story: STORY })
        }
      />
    </SitesChrome>
  );
}
