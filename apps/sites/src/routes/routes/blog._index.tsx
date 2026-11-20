import { BlogIndexView } from "@/pages/blog/BlogIndexView";
import { parseBlogPage } from "@/data/lib/blog/normalize";
import { loadBlogHome } from "@/data/lib/blog/pages";
import "@dcl/one-ui/foundations/foundations.css";

import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { blogHeaders, blogResponse } from "../lib/blog";

import { canonicalLink } from "../lib/canonical";
import type { Route } from "./+types/blog._index";

export function meta({ matches, location }: Route.MetaArgs) {
  const page = parseBlogPage(new URLSearchParams(location.search).get("page"));
  return [
    { title: "Blog | Decentraland" },
    { name: "description", content: "News, creator stories, events, and updates from the Decentraland community." },
    ...canonicalLink(matches[0]?.loaderData?.publicPageBase, page > 1 ? `/blog?page=${page}` : "/blog"),
  ];
}

export const handle = { agentMarkdown: "blogIndex" } satisfies AgentMarkdownHandle;

export const headers = blogHeaders;

export function loader({ request, context }: Route.LoaderArgs) {
  return blogResponse(request, context, loadBlogHome(request));
}

export default function BlogIndexRoute({ loaderData }: Route.ComponentProps) {
  return <BlogIndexView {...loaderData} />;
}
