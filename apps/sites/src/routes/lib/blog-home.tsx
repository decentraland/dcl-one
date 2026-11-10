import { listQuery } from "@/data/lib/blog/options";
import type { BlogDocument, BlogBase } from "@/features/blog/loaders";
import { serverBase } from "@/features/blog/loaders";
import { PostList } from "@/features/blog/BlogShell";
import { BlogRoute } from "@/features/blog/BlogRoute";
import { BlogPage, useBlogNavigate } from "./blog-page";

const HOME = { scope: "home" } as const;
export type HomeData = BlogDocument & Pick<BlogBase, "publicBase">;
export async function homeLoader({ request }: { request: Request }) {
  const { client, base, wrap, required } = await serverBase(
    request,
    "index_grid",
  );
  await required(() => client.fetchInfiniteQuery(listQuery(HOME)));
  return wrap({ publicBase: base.publicBase });
}
export const homeClientLoader = () => null;
function HomeView({ base }: { base: BlogBase }) {
  const navigate = useBlogNavigate();
  return (
    <BlogPage base={base} activeHref="/blog">
      <PostList options={listQuery(HOME)} hasMainPost onNavigate={navigate} />
    </BlogPage>
  );
}
export function BlogHome({ data }: { data: HomeData | null }) {
  return (
    <BlogRoute data={data}>{(base) => <HomeView base={base} />}</BlogRoute>
  );
}
