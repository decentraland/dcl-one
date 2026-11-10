import { useSearchParams } from "react-router";
import { searchQuery } from "@/data/lib/blog/options";
import { SEARCH_MIN_LENGTH } from "@/data/lib/blog/queries";
import { SearchResultsList } from "@/features/blog/BlogShell";
import { BlogRoute } from "@/features/blog/BlogRoute";
import {
  serverBase,
  type BlogDocument,
  type BlogBase,
} from "@/features/blog/loaders";
import { searchSeo } from "@/features/blog/seo";
import {
  BlogErrorBoundary,
  blogHeaders,
  BlogPage,
  useBlogNavigate,
} from "../lib/blog-page";
import type { Route } from "./+types/blog.search";
export async function loader({ request }: Route.LoaderArgs) {
  const { client, base, wrap, required } = await serverBase(
    request,
    "blog_search",
  );
  const q = new URL(request.url).searchParams.get("q") ?? "";
  if (q.length >= SEARCH_MIN_LENGTH)
    await required(() => client.fetchInfiniteQuery(searchQuery(q)));
  return wrap({ publicBase: base.publicBase, q });
}
export const clientLoader = () => null;
export const meta = () => [];
function SearchView({ base }: { base: BlogBase }) {
  const [params] = useSearchParams();
  const q = params.get("q") ?? "";
  const navigate = useBlogNavigate();
  return (
    <BlogPage base={base} seo={searchSeo(q, base.publicBase)}>
      <SearchResultsList q={q} onNavigate={navigate} />
    </BlogPage>
  );
}
export default function SearchRoute({ loaderData }: Route.ComponentProps) {
  return (
    <BlogRoute data={loaderData as BlogDocument | null}>
      {(base) => <SearchView base={base} />}
    </BlogRoute>
  );
}
export const ErrorBoundary = BlogErrorBoundary;
export const headers = blogHeaders;
