import { postSeo } from "@/features/blog/seo";
import { useEffect, useRef } from "react";
import { track } from "@/core/lib/telemetry/track";
import { useBlogSession } from "@/features/blog/telemetry";
import { useParams } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { postQuery } from "@/data/lib/blog/options";
import { BlogRoute } from "@/features/blog/BlogRoute";
import {
  serverBase,
  type BlogDocument,
  type BlogBase,
} from "@/features/blog/loaders";
import { useRelatedPosts } from "@/features/blog/related";
import { blogCanonical } from "@/features/blog/route-kit";
import { BlogPostArticle } from "@dcl/one-ui/apps/sites/blog/BlogPostArticle";
import { BlogRelatedPosts } from "@dcl/one-ui/apps/sites/blog/BlogRelatedPosts";
import { BlogRichText } from "@dcl/one-ui/apps/sites/blog/BlogRichText";
import {
  BlogPage,
  BlogErrorBoundary,
  blogHeaders,
  useBlogNavigate,
} from "../lib/blog-page";
import type { Route } from "./+types/blog.$categorySlug.$postSlug";
export async function loader({ request, params }: Route.LoaderArgs) {
  const { client, base, wrap, required } = await serverBase(
    request,
    "blog_post",
  );
  await required(() =>
    client.fetchQuery(postQuery(params.categorySlug, params.postSlug)),
  );
  return wrap({ publicBase: base.publicBase });
}
export const clientLoader = () => null;
function Article({
  post,
  base,
}: {
  post: NonNullable<ReturnType<typeof usePost>["data"]>;
  base: BlogBase;
}) {
  const session = useBlogSession();
  const viewed = useRef("");
  useEffect(() => {
    if (viewed.current !== post.slug) {
      viewed.current = post.slug;
      track("lp_blog_post_viewed", { slug: post.slug }, session);
    }
  }, [post.slug, session]);
  const related = useRelatedPosts(post);
  const navigate = useBlogNavigate();
  return (
    <BlogPage
      base={base}
      seo={postSeo(post, base.publicBase)}
      activeHref={post.category.url}
      after={<BlogRelatedPosts posts={related} onNavigate={navigate} />}
    >
      <BlogPostArticle
        post={post}
        shareUrl={blogCanonical(base.publicBase, post.url)}
        onNavigate={navigate}
      >
        <BlogRichText
          body={post.body}
          assets={post.bodyAssets}
          blogBaseUrl={`${base.publicBase}/blog`}
          onNavigate={navigate}
        />
      </BlogPostArticle>
    </BlogPage>
  );
}
function usePost() {
  const { categorySlug = "", postSlug = "" } = useParams();
  return useQuery({ ...postQuery(categorySlug, postSlug), throwOnError: true });
}
function PostView({ base }: { base: BlogBase }) {
  const post = usePost();
  return post.data ? (
    <Article post={post.data} base={base} />
  ) : (
    <BlogPage base={base}>
      <p role="status">Loading post…</p>
    </BlogPage>
  );
}
export default function PostRoute({ loaderData }: Route.ComponentProps) {
  return (
    <BlogRoute data={loaderData as BlogDocument | null}>
      {(base) => <PostView base={base} />}
    </BlogRoute>
  );
}
export const ErrorBoundary = BlogErrorBoundary;
export const headers = blogHeaders;

export const handle = { agentMarkdown: "blogArticle" } as const;
export const meta = () => [];
