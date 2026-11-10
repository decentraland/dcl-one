import { BlogHead } from "./blog-head";
import { homeSeo } from "@/features/blog/seo";
import { blogPublicBase, type BlogSeo } from "@/features/blog/route-kit";
import { track } from "@/core/lib/telemetry/track";
import { useQueryClient } from "@tanstack/react-query";
import { BlogMissingError } from "@/data/lib/blog/options";
// The blog route shell (blog plan B1, R4-3): localized chrome around an English blog region.
import { useCallback, useEffect, useMemo, type ReactNode } from "react";
import {
  Link,
  isRouteErrorResponse,
  useLocation,
  useNavigate,
  useNavigation,
  useRouteError,
  useRouteLoaderData,
  useRevalidator,
} from "react-router";

import {
  BlogLayout,
  BlogNavigation,
} from "@dcl/one-ui/apps/sites/blog/BlogLayout";
import { BlogPostArticleSkeleton } from "@dcl/one-ui/apps/sites/blog/BlogPostArticle";
import { BlogPostGrid } from "@dcl/one-ui/apps/sites/blog/BlogPostGrid";
import { BlogSearch } from "@dcl/one-ui/apps/sites/blog/BlogSearch";

import type { StoryId } from "@/core/lib/telemetry/story-id";
import type { BlogBase } from "@/features/blog/loaders";
import { useBlogSearchBox } from "@/features/blog/search-box";
import {
  BLOG_NOT_FOUND_COPY,
  blogNotFoundMeta,
  type BlogMissing,
} from "@/features/blog/route-kit";
import { LocaleProvider, useLocale } from "@/features/locale/LocaleProvider";
import { DEFAULT_LOCALE } from "@/features/locale/locales";

import { ErrorBoundary as RootErrorBoundary } from "../root";

import SitesPageChrome from "./sites-page-chrome";

export const BLOG_STORY: StoryId = "misc/blog";

function BlogRegion({
  children,
  busy,
}: {
  children: ReactNode;
  busy: boolean;
}) {
  const { locale } = useLocale();
  return (
    <main
      lang={locale === DEFAULT_LOCALE ? undefined : DEFAULT_LOCALE}
      data-blog-region=""
      aria-busy={busy || undefined}
    >
      {children}
    </main>
  );
}

/** The visit's sid, from the root loader (the same value the page's server loader issued). */
function useBlogSid(): string {
  const root = useRouteLoaderData("root") as { sid?: string } | undefined;
  return root?.sid ?? "";
}

type BlogPageProps = {
  base: BlogBase;
  seo?: BlogSeo;
  noIndex?: boolean;
  /** The navigation entry to mark: `/blog`, or a category's url. */
  activeHref?: string;
  /** Full width, between the navigation and the content (a category's hero). */
  banner?: ReactNode;
  /** Full width, after the content (a post's related rail). */
  after?: ReactNode;
  children: ReactNode;
};

/** In-app navigation for the blog's links (plain clicks only; see blogLinks.inAppLink). */
export function useBlogNavigate(): (href: string) => void {
  const navigate = useNavigate();
  const sid = useBlogSid();
  return (href) => {
    const match = /^\/blog\/[^/]+\/([^/?#]+)(?:[?#]|$)/.exec(href);
    if (match && !href.startsWith("/blog/author/"))
      track(
        "lp_blog_post_clicked",
        { slug: match[1]! },
        { sid, story: BLOG_STORY },
      );
    void navigate(href);
  };
}

const POST_PATH = /^\/blog\/(?!author\/|search$|sign-in$)[^/]+\/[^/]+\/?$/;

/**
 * The loading state while a client navigation to another blog page waits for its data (source shows
 * a spinner while a post, category or author loads): a post's skeleton for a post, the list's for a
 * list. Null when nothing is pending, or the pending location is this page (a revalidation).
 */
const HERO_LIST_PATH = /^\/blog(?:\/(?!author\/|search$|sign-in$)[^/]+)?\/?$/;

function usePendingBlogPage(): "post" | "hero-list" | "list" | null {
  const navigation = useNavigation();
  const current = useLocation();
  const target = navigation.location;
  if (
    navigation.state !== "loading" ||
    !target ||
    !/^\/blog(\/|$)/.test(target.pathname)
  )
    return null;
  if (target.pathname.startsWith("/blog/sign-in")) return null;
  if (target.pathname === current.pathname && target.search === current.search)
    return null;
  if (POST_PATH.test(target.pathname)) return "post";
  return HERO_LIST_PATH.test(target.pathname) ? "hero-list" : "list";
}

export function BlogPage({
  base,
  seo,
  noIndex,
  activeHref,
  banner,
  after,
  children,
}: BlogPageProps) {
  const sid = useBlogSid();
  const onNavigate = useBlogNavigate();
  const pending = usePendingBlogPage();
  const categories = base.categories.filter(
    (category) => category.isShownInMenu,
  );
  return (
    <LocaleProvider initial={base.messages}>
      <BlogHead seo={seo ?? homeSeo(base.publicBase)} noIndex={noIndex} />
      <BlogChrome sid={sid}>
        <BlogRegion busy={pending !== null}>
          <BlogLayout
            navigation={
              <BlogNavigation
                categories={categories}
                activeHref={activeHref}
                search={<BlogSearchBox onNavigate={onNavigate} />}
                onNavigate={onNavigate}
              />
            }
            banner={pending ? undefined : banner}
            after={pending ? undefined : after}
          >
            {pending === "post" ? (
              <BlogPostArticleSkeleton />
            ) : pending ? (
              <BlogPostGrid
                posts={[]}
                loading
                hasMainPost={pending === "hero-list"}
              />
            ) : (
              children
            )}
          </BlogLayout>
        </BlogRegion>
      </BlogChrome>
    </LocaleProvider>
  );
}

function BlogSearchBox({ onNavigate }: { onNavigate: (href: string) => void }) {
  return <BlogSearch {...useBlogSearchBox()} onNavigate={onNavigate} />;
}

function BlogChrome({ sid, children }: { sid: string; children: ReactNode }) {
  const session = useMemo(() => ({ sid, story: BLOG_STORY }), [sid]);
  return (
    <SitesPageChrome session={session} header={{ variant: "full" }}>
      {children}
    </SitesPageChrome>
  );
}

type MissingData = { kind?: BlogMissing; base?: BlogBase };

const missing = (error: unknown): MissingData | null =>
  isRouteErrorResponse(error) &&
  error.status === 404 &&
  error.data &&
  typeof error.data === "object"
    ? (error.data as MissingData)
    : null;

/** Route meta for a missing post, category or author; null for any other outcome. */
export function blogErrorMeta(error: unknown) {
  const data = missing(error);
  return data?.kind ? blogNotFoundMeta(data.kind) : null;
}

/**
 * A blog route's error boundary: a missing post, category or author shows the source's copy inside
 * the page chrome (R4-2). Anything else (the 503 of an unavailable source included) shows root's
 * error view.
 */
export function BlogErrorBoundary() {
  const error = useRouteError();
  const location = useLocation();
  const errorSeo = {
    title: "Blog unavailable",
    description: "The blog is temporarily unavailable.",
    canonical: `${blogPublicBase()}${location.pathname}`,
  };
  const client = useQueryClient();
  const revalidator = useRevalidator();
  const reset = useCallback(
    () =>
      client.removeQueries({
        predicate: (query) =>
          query.queryKey[0] === "blog" && query.state.status === "error",
      }),
    [client],
  );
  useEffect(() => () => reset(), [reset]);
  const retry = () => {
    reset();
    void revalidator.revalidate();
  };
  const data = missing(error);
  if (error instanceof BlogMissingError)
    return (
      <main>
        <BlogHead seo={{ ...errorSeo, title: "Blog page not found" }} noIndex />
        <p role="alert">{BLOG_NOT_FOUND_COPY[error.kind]}</p>
        <Link to="/blog">Back to the blog</Link>
        <button type="button" onClick={retry}>
          Try again
        </button>
      </main>
    );
  // Not rethrown: during SSR a parent boundary does not catch an error thrown by this one.
  if (!data?.kind || !data.base)
    return (
      <>
        <BlogHead seo={errorSeo} noIndex />
        <RootErrorBoundary
          {...({ error } as Parameters<typeof RootErrorBoundary>[0])}
        />
        <button type="button" onClick={retry}>
          Try again
        </button>
      </>
    );
  return (
    <BlogPage
      base={data.base}
      seo={{ ...errorSeo, title: "Blog page not found" }}
      noIndex
    >
      <p role="alert">{BLOG_NOT_FOUND_COPY[data.kind]}</p>
    </BlogPage>
  );
}

// The thrown answers' own headers that a document keeps: no-store and noindex on the R4-2 404,
// no-store and Retry-After on the 503. React Router merges only cookies unless the boundary route
// exports headers (measured: without this, the 503 document lost both).
const ERROR_HEADERS = ["Cache-Control", "X-Robots-Tag", "Retry-After"];

export function blogHeaders({ errorHeaders }: { errorHeaders?: Headers }) {
  const headers = new Headers();
  for (const name of ERROR_HEADERS) {
    const value = errorHeaders?.get(name);
    if (value) headers.set(name, value);
  }
  return headers;
}
