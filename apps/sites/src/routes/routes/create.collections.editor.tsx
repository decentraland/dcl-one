import { pageLoader, type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { creatorHubMeta } from "@/core/lib/seo/creator-hub-meta";
import CreatorsShell from "@/features/creators/CreatorsShell";

import type { Route } from "./+types/create.collections.editor";

export const meta = () => creatorHubMeta("Item editor");

const FALLBACK: PageDefaults = { variant: "item-editor" };

// The page's data loads in the browser: builder-server needs requests signed by the visitor's session.
export async function loader({ request, context }: Route.LoaderArgs) {
  const { sid, wrap } = await pageLoader(request, context, FALLBACK);
  return wrap({ sid });
}

export default function CreateItemeditorRoute({ loaderData }: Route.ComponentProps) {
  return <CreatorsShell page="editor" sid={loaderData.sid} />;
}
