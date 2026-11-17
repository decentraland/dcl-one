// @vitest-environment jsdom
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentType, ReactNode } from "react";
import { createRoutesStub, redirect, useLoaderData } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

const track = vi.hoisted(() => vi.fn());
vi.mock("@/core/lib/telemetry/track", () => ({ track }));
vi.mock("@ui/web/frames/SitesChrome", () => ({ default: ({ children }: { children?: ReactNode }) => <>{children}</> }));

import { BlogIndexView } from "@/features/blog/BlogIndexView";
import { BlogPostView } from "@/features/blog/BlogPostView";
import { installFakeCms } from "@/data/lib/blog/cms.test-utils";
import { loadBlogAuthor, loadBlogCategory, loadBlogHome, loadBlogPost, loadBlogSearch, type BlogOutcome } from "@/data/lib/blog/pages";

// The framework's Vite plugin wraps the route modules' components and drops their loaders from this
// client-side environment, so render the views the routes render with the data the loaders send.
function withLoaderData<P extends object>(View: ComponentType<P>) {
  return function Routed() {
    return <View {...(useLoaderData() as P)} />;
  };
}

async function payload<T>(load: Promise<BlogOutcome<T>>) {
  const outcome = await load;
  if ("location" in outcome) throw redirect(outcome.location);
  return { sid: "s", ...outcome.payload };
}

const Stub = createRoutesStub([
  { path: "/blog", Component: withLoaderData(BlogIndexView), loader: ({ request }) => payload(loadBlogHome(request)), HydrateFallback: () => null },
  { path: "/blog/search", Component: withLoaderData(BlogIndexView), loader: ({ request }) => payload(loadBlogSearch(request)), HydrateFallback: () => null },
  {
    path: "/blog/author/:author",
    Component: withLoaderData(BlogIndexView),
    loader: ({ request, params }) => payload(loadBlogAuthor(request, params.author ?? "")),
    HydrateFallback: () => null,
  },
  {
    path: "/blog/:category",
    Component: withLoaderData(BlogIndexView),
    loader: ({ request, params }) => payload(loadBlogCategory(request, params.category ?? "")),
    HydrateFallback: () => null,
  },
  {
    path: "/blog/:category/:post",
    Component: withLoaderData(BlogPostView),
    loader: ({ request, params }) => payload(loadBlogPost(request, params.category ?? "", params.post ?? "")),
    HydrateFallback: () => null,
  },
]);

const calls = (event: string) => track.mock.calls.filter(([name]) => name === event).map(([, props]) => props);

function renderAt(url: string) {
  return render(<Stub initialEntries={[url]} />);
}

beforeEach(() => {
  installFakeCms();
});

afterEach(() => {
  cleanup();
  track.mockClear();
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
});

describe("blog list telemetry", () => {
  it("fires lp_blog_viewed once with the CMS total, and not again for Load more or a search", async () => {
    const user = userEvent.setup();
    renderAt("/blog");
    await screen.findByRole("heading", { level: 1, name: "The Decentraland Blog" });
    await waitFor(() =>
      expect(calls("lp_blog_viewed")).toEqual([{ post_count: 11 }]),
    );

    await user.click(screen.getByRole("link", { name: "Load more" }));
    await screen.findByRole("heading", { level: 3, name: "News 9" });
    expect(screen.queryByRole("link", { name: "Load more" })).toBeNull();

    const [box] = screen.getAllByRole("searchbox", { name: "Search articles" });
    await user.type(box!, "news{Enter}");
    await screen.findByText("9 articles for “news”");
    expect(calls("lp_blog_viewed")).toEqual([{ post_count: 11 }]);
  });

  it("fires it for each category and author view, with that list's total", async () => {
    const user = userEvent.setup();
    renderAt("/blog");
    await screen.findByRole("heading", { level: 1, name: "The Decentraland Blog" });
    const nav = screen.getByRole("navigation", { name: "Blog categories" });
    await user.click(within(nav).getByRole("link", { name: "Announcements" }));
    await screen.findByRole("heading", { level: 1, name: "Announcements" });
    await waitFor(() =>
      expect(calls("lp_blog_viewed")).toEqual([
        { post_count: 11 },
        { post_count: 9 },
      ]),
    );

    cleanup();
    track.mockClear();
    renderAt("/blog/author/jane-doe");
    await screen.findByRole("heading", { level: 1, name: "Jane Doe" });
    await waitFor(() =>
      expect(calls("lp_blog_viewed")).toEqual([{ post_count: 1 }]),
    );
  });

  it("fires nothing for an unknown category or an unavailable blog", async () => {
    renderAt("/blog/nope");
    await screen.findByRole("heading", { level: 1, name: "Category not found" });
    cleanup();
    vi.stubEnv("BLOG_CMS_URL", "");
    renderAt("/blog");
    await screen.findByRole("heading", { level: 1, name: "The blog is temporarily unavailable" });
    expect(track).not.toHaveBeenCalled();
  });

  it("fires lp_blog_post_clicked with the slug from the featured card and a grid card, and not from a tab", async () => {
    const user = userEvent.setup();
    renderAt("/blog");
    await user.click(await screen.findByRole("link", { name: "First Post" }));
    await screen.findByRole("heading", { level: 1, name: "First Post" });
    expect(calls("lp_blog_post_clicked")).toEqual([{ slug: "first-post" }]);

    track.mockClear();
    cleanup();
    renderAt("/blog");
    const nav = await screen.findByRole("navigation", { name: "Blog categories" });
    await user.click(within(nav).getByRole("link", { name: "Platform" }));
    await screen.findByRole("heading", { level: 1, name: "Platform" });
    await user.click(await screen.findByRole("link", { name: "Second Post" }));
    await screen.findByRole("heading", { level: 1, name: "Second Post" });
    expect(calls("lp_blog_post_clicked")).toEqual([{ slug: "second-post" }]);
  });
});

describe("blog post telemetry", () => {
  it("fires lp_blog_post_viewed once, and lp_blog_post_clicked plus a new view for a related card", async () => {
    const user = userEvent.setup();
    renderAt("/blog/platform/first-post");
    await screen.findByRole("heading", { level: 1, name: "First Post" });
    await waitFor(() =>
      expect(calls("lp_blog_post_viewed")).toEqual([{ slug: "first-post" }]),
    );

    const related = screen.getByRole("region", { name: /More on|More from/ });
    await user.click(within(related).getByRole("link", { name: "Second Post" }));
    await screen.findByRole("heading", { level: 1, name: "Second Post" });

    await waitFor(() => {
      expect(calls("lp_blog_post_clicked")).toEqual([{ slug: "second-post" }]);
      expect(calls("lp_blog_post_viewed")).toEqual([
        { slug: "first-post" },
        { slug: "second-post" },
      ]);
    });
  });

  it("sends no click for the back link, the author link or a link in the body, and no view for a missing post", async () => {
    const user = userEvent.setup();
    renderAt("/blog/platform/first-post");
    const article = (await screen.findByRole("heading", { level: 1, name: "First Post" })).closest("article")!;
    expect(within(article).getByRole("link", { name: "the docs" }).getAttribute("target")).toBe("_blank");
    await user.click(within(article).getByRole("link", { name: "Jane Doe" }));
    await screen.findByRole("heading", { level: 1, name: "Jane Doe" });
    expect(calls("lp_blog_post_clicked")).toEqual([]);

    cleanup();
    renderAt("/blog/platform/first-post");
    await user.click(await screen.findByRole("link", { name: "Back to blog" }));
    await screen.findByRole("heading", { level: 1, name: "The Decentraland Blog" });
    expect(calls("lp_blog_post_clicked")).toEqual([]);

    cleanup();
    track.mockClear();
    renderAt("/blog/platform/nope");
    await screen.findByRole("heading", { level: 1, name: "Post not found" });
    expect(track).not.toHaveBeenCalled();
  });

  it("renders no blocks the CMS has no data for (takeaways, CTA, FAQ, author bio)", async () => {
    renderAt("/blog/platform/first-post");
    const article = (await screen.findByRole("heading", { level: 1 })).closest("article")!;
    for (const name of [/takeaway/i, /faq|frequently asked/i, /about the author/i, /get started|sign up|download/i]) {
      expect(within(article).queryByRole("heading", { name })).toBeNull();
    }
    expect(within(article).queryByRole("button", { name: /get started|sign up|download/i })).toBeNull();
    expect(article.querySelector("details, dl")).toBeNull();
  });
});
