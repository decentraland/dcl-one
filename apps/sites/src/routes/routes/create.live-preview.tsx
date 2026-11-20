import { pageLoader, type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { creatorHubMeta } from "@/core/lib/seo/creator-hub-meta";
import CreatorsShell from "@/features/creators/CreatorsShell";

import type { Route } from "./+types/create.live-preview";

export const meta = () => creatorHubMeta("Live preview");

const FALLBACK: PageDefaults = { variant: "live-preview" };

// The page's data loads in the browser: builder-server needs requests signed by the visitor's session.
export async function loader({ request, context }: Route.LoaderArgs) {
  const { sid, wrap } = await pageLoader(request, context, FALLBACK);
  return wrap({ sid });
}

export default function CreateLivepreviewRoute({ loaderData }: Route.ComponentProps) {
  return <CreatorsShell page="livePreview" sid={loaderData.sid} />;
}
