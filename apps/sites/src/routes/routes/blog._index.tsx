import { BlogIndexView } from "@/features/blog/BlogIndexView";
import { blogIndexData } from "@/features/blog/data";
import "@dcl/one-ui/foundations/foundations.css";

import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";

import type { Route } from "./+types/blog._index";

export function meta() {
  return [{ title: "Decentraland Blog" }, { name: "description", content: "News, creator stories, events, and updates from the Decentraland community." }];
}

export const handle = { agentMarkdown: "blogIndex" } satisfies AgentMarkdownHandle;

const FALLBACK: PageDefaults = {
  variant: "index_grid",
};

export async function loader({ request }: Route.LoaderArgs) {
  const { sid, wrap } = await pageLoader(request, FALLBACK);
  return wrap({ sid, ...blogIndexData(new URL(request.url)) });
}

export default function BlogIndexRoute({ loaderData }: Route.ComponentProps) {
  return <BlogIndexView {...loaderData} />;
}
