// @vitest-environment jsdom
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import userEvent from "@testing-library/user-event";
import type { ComponentType, ReactNode } from "react";
import { createRoutesStub, redirect, useLoaderData } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

const track = vi.hoisted(() => vi.fn());
vi.mock("@/core/lib/telemetry/track", () => ({ track }));
vi.mock("@/features/components/chrome/SitesLayout", () => ({ default: ({ children }: { children?: ReactNode }) => <>{children}</> }));

import { BlogIndexView } from "@/pages/blog/BlogIndexView";
import { BlogPostView } from "@/pages/blog/BlogPostView";
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
  return render(
    <QueryClientProvider client={new QueryClient()}>
      <Stub initialEntries={[url]} />
    </QueryClientProvider>,
  );
}

const BLOG_HOME_HEADING = { level: 1, name: "The Decentraland Blog" } as const;

describe("blog telemetry", () => {
  let user: ReturnType<typeof userEvent.setup>;

  beforeEach(() => {
    installFakeCms();
    user = userEvent.setup();
  });

  afterEach(() => {
    cleanup();
    vi.resetAllMocks();
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
  });

  describe("when the blog home is viewed", () => {
    beforeEach(async () => {
      renderAt("/blog");
      await screen.findByRole("heading", BLOG_HOME_HEADING);
    });

    it("should fire lp_blog_viewed once with the CMS total", async () => {
      await waitFor(() =>
        expect(calls("lp_blog_viewed")).toEqual([{ post_count: 11 }]),
      );
    });

    describe("and the reader loads more and then searches", () => {
      beforeEach(async () => {
        await waitFor(() => expect(calls("lp_blog_viewed")).toHaveLength(1));
        await user.click(screen.getByRole("link", { name: "Load more" }));
        await screen.findByRole("heading", { level: 3, name: "News 9" });
      });

      it("should load the last page, leaving no Load more, and not fire lp_blog_viewed again", async () => {
        expect(screen.queryByRole("link", { name: "Load more" })).toBeNull();

        const box = screen.getByRole("combobox", { name: "Search articles" });
        await user.type(box, "news{Enter}");
        await screen.findByText("9 articles for “news”");
        expect(calls("lp_blog_viewed")).toEqual([{ post_count: 11 }]);
      });
    });

    describe("and the reader opens a category", () => {
      beforeEach(async () => {
        const nav = screen.getByRole("navigation", { name: "Blog categories" });
        await user.click(within(nav).getByRole("link", { name: "Announcements" }));
        await screen.findByRole("heading", { level: 1, name: "Announcements" });
      });

      it("should fire lp_blog_viewed for the category with its total", async () => {
        await waitFor(() =>
          expect(calls("lp_blog_viewed")).toEqual([
            { post_count: 11 },
            { post_count: 9 },
          ]),
        );
      });
    });

    describe("and the reader clicks the featured card", () => {
      beforeEach(async () => {
        await user.click(await screen.findByRole("link", { name: "First Post" }));
        await screen.findByRole("heading", { level: 1, name: "First Post" });
      });

      it("should fire lp_blog_post_clicked with its slug", () => {
        expect(calls("lp_blog_post_clicked")).toEqual([{ slug: "first-post" }]);
      });
    });

    describe("and the reader opens a category tab and clicks a grid card", () => {
      beforeEach(async () => {
        const nav = await screen.findByRole("navigation", { name: "Blog categories" });
        await user.click(within(nav).getByRole("link", { name: "Platform" }));
        await screen.findByRole("heading", { level: 1, name: "Platform" });
        await user.click(await screen.findByRole("link", { name: "Second Post" }));
        await screen.findByRole("heading", { level: 1, name: "Second Post" });
      });

      it("should fire lp_blog_post_clicked with the card's slug, and not for the tab", () => {
        expect(calls("lp_blog_post_clicked")).toEqual([{ slug: "second-post" }]);
      });
    });
  });

  describe("when an author's list is viewed", () => {
    beforeEach(async () => {
      renderAt("/blog/author/jane-doe");
      await screen.findByRole("heading", { level: 1, name: "Jane Doe" });
    });

    it("should fire lp_blog_viewed with that list's total", async () => {
      await waitFor(() =>
        expect(calls("lp_blog_viewed")).toEqual([{ post_count: 1 }]),
      );
    });
  });

  describe("when an unknown category is viewed", () => {
    beforeEach(async () => {
      renderAt("/blog/nope");
      await screen.findByRole("heading", { level: 1, name: "Category not found" });
    });

    describe("and then the blog becomes unavailable", () => {
      beforeEach(async () => {
        cleanup();
        vi.stubEnv("BLOG_CMS_URL", "");
        renderAt("/blog");
        await screen.findByRole("heading", { level: 1, name: "The blog is temporarily unavailable" });
      });

      it("should fire nothing for either", () => {
        expect(track).not.toHaveBeenCalled();
      });
    });
  });

  describe("when a post is viewed", () => {
    let article: HTMLElement;

    beforeEach(async () => {
      renderAt("/blog/platform/first-post");
      article = (await screen.findByRole("heading", { level: 1, name: "First Post" })).closest("article")!;
    });

    it("should fire lp_blog_post_viewed once", async () => {
      await waitFor(() =>
        expect(calls("lp_blog_post_viewed")).toEqual([{ slug: "first-post" }]),
      );
    });

    it("should open a link in the body in a new tab", () => {
      expect(within(article).getByRole("link", { name: "the docs" }).getAttribute("target")).toBe("_blank");
    });

    it("should render no blocks the CMS has no data for (takeaways, CTA, FAQ, author bio)", () => {
      for (const name of [/takeaway/i, /faq|frequently asked/i, /about the author/i, /get started|sign up|download/i]) {
        expect(within(article).queryByRole("heading", { name })).toBeNull();
      }
      expect(within(article).queryByRole("button", { name: /get started|sign up|download/i })).toBeNull();
      expect(article.querySelector("details, dl")).toBeNull();
    });

    describe("and the reader clicks a related card", () => {
      beforeEach(async () => {
        await waitFor(() => expect(calls("lp_blog_post_viewed")).toHaveLength(1));
        const related = screen.getByRole("region", { name: /More on|More from/ });
        await user.click(within(related).getByRole("link", { name: "Second Post" }));
        await screen.findByRole("heading", { level: 1, name: "Second Post" });
      });

      it("should fire lp_blog_post_clicked and a new view", async () => {
        await waitFor(() => {
          expect(calls("lp_blog_post_clicked")).toEqual([{ slug: "second-post" }]);
          expect(calls("lp_blog_post_viewed")).toEqual([
            { slug: "first-post" },
            { slug: "second-post" },
          ]);
        });
      });
    });

    describe("and the reader clicks the author link", () => {
      beforeEach(async () => {
        await user.click(within(article).getByRole("link", { name: "Jane Doe" }));
        await screen.findByRole("heading", { level: 1, name: "Jane Doe" });
      });

      it("should send no click", () => {
        expect(calls("lp_blog_post_clicked")).toEqual([]);
      });
    });

    describe("and the reader clicks the back link", () => {
      beforeEach(async () => {
        await user.click(await screen.findByRole("link", { name: "Back to blog" }));
        await screen.findByRole("heading", BLOG_HOME_HEADING);
      });

      it("should send no click", () => {
        expect(calls("lp_blog_post_clicked")).toEqual([]);
      });
    });
  });

  describe("when a missing post is viewed", () => {
    beforeEach(async () => {
      renderAt("/blog/platform/nope");
      await screen.findByRole("heading", { level: 1, name: "Post not found" });
    });

    it("should send no view", () => {
      expect(track).not.toHaveBeenCalled();
    });
  });
});
