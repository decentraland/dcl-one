import { pageLoader, type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { creatorHubMeta } from "@/core/lib/seo/creator-hub-meta";
import CreatorsShell from "@/features/creators/CreatorsShell";

import type { Route } from "./+types/create.curation";

export const meta = () => creatorHubMeta("Curation");

const FALLBACK: PageDefaults = { variant: "curation" };

// The page's data loads in the browser: builder-server needs requests signed by the visitor's session.
export async function loader({ request, context }: Route.LoaderArgs) {
  const { sid, wrap } = await pageLoader(request, context, FALLBACK);
  return wrap({ sid });
}

export default function CreateCurationRoute({ loaderData }: Route.ComponentProps) {
  return <CreatorsShell page="curation" sid={loaderData.sid} />;
}
