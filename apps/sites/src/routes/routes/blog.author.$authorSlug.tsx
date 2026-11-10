import { authorSeo } from "@/features/blog/seo";

import { useParams } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { authorQuery, listQuery } from "@/data/lib/blog/options";
import { PostList } from "@/features/blog/BlogShell";
import { BlogRoute } from "@/features/blog/BlogRoute";
import {
  serverBase,
  type BlogDocument,
  type BlogBase,
} from "@/features/blog/loaders";
import { BlogAuthorHeader } from "@dcl/one-ui/apps/sites/blog/BlogListHeaders";
import {
  BlogPage,
  BlogErrorBoundary,
  blogHeaders,
  useBlogNavigate,
} from "../lib/blog-page";
import type { Route } from "./+types/blog.author.$authorSlug";
export async function loader({ request, params }: Route.LoaderArgs) {
  const { client, base, wrap, required } = await serverBase(
    request,
    "blog_author",
  );
  await required(() => client.fetchQuery(authorQuery(params.authorSlug)));
  await required(() =>
    client.fetchInfiniteQuery(
      listQuery({ scope: "author", slug: params.authorSlug }),
    ),
  );
  return wrap({ publicBase: base.publicBase });
}
export const clientLoader = () => null;
function AuthorView({ base }: { base: BlogBase }) {
  const { authorSlug = "" } = useParams();
  const author = useQuery({ ...authorQuery(authorSlug), throwOnError: true });
  const navigate = useBlogNavigate();
  return (
    <BlogPage
      base={base}
      seo={author.data ? authorSeo(author.data, base.publicBase) : undefined}
    >
      {author.data ? (
        <>
          <BlogAuthorHeader
            name={author.data.title}
            description={author.data.description}
            image={author.data.image.url}
          />
          <PostList
            options={listQuery({ scope: "author", slug: authorSlug })}
            onNavigate={navigate}
          />
        </>
      ) : (
        <p role="status">Loading author…</p>
      )}
    </BlogPage>
  );
}
export default function AuthorRoute({ loaderData }: Route.ComponentProps) {
  return (
    <BlogRoute data={loaderData as BlogDocument | null}>
      {(base) => <AuthorView base={base} />}
    </BlogRoute>
  );
}
export const ErrorBoundary = BlogErrorBoundary;
export const headers = blogHeaders;

export const meta = () => [];
