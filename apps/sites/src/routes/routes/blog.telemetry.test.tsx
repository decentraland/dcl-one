// @vitest-environment jsdom
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentType, ReactNode } from "react";
import { createRoutesStub, useLoaderData } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

const track = vi.hoisted(() => vi.fn());
vi.mock("@/core/lib/telemetry/track", () => ({ track }));
vi.mock("@ui/web/frames/SitesChrome", () => ({ default: ({ children }: { children?: ReactNode }) => <>{children}</> }));

import { BlogIndexView } from "@/features/blog/BlogIndexView";
import { BlogPostView } from "@/features/blog/BlogPostView";
import { blogIndexData, blogPostData } from "@/features/blog/data";

// The framework's Vite plugin wraps the route modules' components and drops their loaders from this
// client-side environment, so render the views the routes render with the data the loaders send.
function withLoaderData<P extends object>(View: ComponentType<P>) {
  return function Routed() {
    return <View {...(useLoaderData() as P)} />;
  };
}

const Stub = createRoutesStub([
  {
    path: "/blog",
    Component: withLoaderData(BlogIndexView),
    loader: ({ request }) => ({ sid: "s", ...blogIndexData(new URL(request.url)) }),
    HydrateFallback: () => null,
  },
  {
    path: "/blog/:slug",
    Component: withLoaderData(BlogPostView),
    loader: ({ request, params }) => ({ sid: "s", ...blogPostData(request, params.slug ?? "") }),
    HydrateFallback: () => null,
  },
]);

const calls = (event: string) => track.mock.calls.filter(([name]) => name === event).map(([, props]) => props);

function renderAt(url: string) {
  return render(<Stub initialEntries={[url]} />);
}

afterEach(() => {
  cleanup();
  track.mockClear();
});

describe("blog index telemetry", () => {
  it("fires lp_blog_viewed once with the category's post count, not the search's", async () => {
    renderAt("/blog?q=zzzz");
    await screen.findByRole("heading", { name: "No articles found" });
    expect(calls("lp_blog_viewed")).toEqual([{ post_count: 3 }]);
  });

  it("does not fire lp_blog_viewed again on a search or a sort, and fires it for each category view", async () => {
    const user = userEvent.setup();
    renderAt("/blog");
    await screen.findByRole("heading", { level: 1, name: "The Decentraland Blog" });

    const [box] = screen.getAllByRole("searchbox", { name: "Search articles" });
    await user.type(box!, "platform{Enter}");
    await screen.findByText("2 articles for “platform”");

    await user.selectOptions(screen.getByRole("combobox", { name: "Sort:" }), "oldest");
    await screen.findByRole("combobox", { name: "Sort:" });
    expect(calls("lp_blog_viewed")).toEqual([{ post_count: 3 }]);

    const nav = screen.getByRole("navigation", { name: "Blog categories" });
    await user.click(within(nav).getByRole("link", { name: "Announcements" }));
    await screen.findByRole("heading", { level: 2, name: "Marketplace Credits Season One Is Live" });

    // Main loaded a new page per tab; the category's own count is reported, once.
    expect(calls("lp_blog_viewed")).toEqual([{ post_count: 3 }, { post_count: 1 }]);
  });

  it("fires lp_blog_post_clicked with the slug from the featured card and a grid card, and not from a tab", async () => {
    const user = userEvent.setup();
    renderAt("/blog");
    await user.click(await screen.findByRole("link", { name: "Explore Decentraland From Your Browser" }));
    await screen.findByRole("heading", { level: 1, name: "Explore Decentraland From Your Browser" });
    expect(calls("lp_blog_post_clicked")).toEqual([{ slug: "explore-from-your-browser" }]);

    track.mockClear();
    cleanup();
    renderAt("/blog");
    const nav = await screen.findByRole("navigation", { name: "Blog categories" });
    await user.click(within(nav).getByRole("link", { name: "Platform" }));
    await user.click(await screen.findByRole("link", { name: "What's On: A Live Guide to Events Across the World" }));
    await screen.findByRole("heading", { level: 1, name: "What's On: A Live Guide to Events Across the World" });
    expect(calls("lp_blog_post_clicked")).toEqual([{ slug: "whats-on-live-events-guide" }]);
  });
});

describe("blog post telemetry", () => {
  it("fires lp_blog_post_viewed once, and lp_blog_post_clicked plus a new view for a related card", async () => {
    const user = userEvent.setup();
    renderAt("/blog/explore-from-your-browser");
    await screen.findByRole("heading", { level: 1, name: "Explore Decentraland From Your Browser" });
    expect(calls("lp_blog_post_viewed")).toEqual([{ slug: "explore-from-your-browser" }]);

    const related = screen.getByRole("region", { name: /More on|More from/ });
    await user.click(within(related).getByRole("link", { name: "What's On: A Live Guide to Events Across the World" }));
    await screen.findByRole("heading", { level: 1, name: "What's On: A Live Guide to Events Across the World" });

    expect(calls("lp_blog_post_clicked")).toEqual([{ slug: "whats-on-live-events-guide" }]);
    expect(calls("lp_blog_post_viewed")).toEqual([{ slug: "explore-from-your-browser" }, { slug: "whats-on-live-events-guide" }]);
  });

  it("sends no event for the back link and no view for a missing post", async () => {
    const user = userEvent.setup();
    renderAt("/blog/explore-from-your-browser");
    await user.click(await screen.findByRole("link", { name: "Back to blog" }));
    await screen.findByRole("heading", { level: 1, name: "The Decentraland Blog" });
    expect(calls("lp_blog_post_clicked")).toEqual([]);

    cleanup();
    track.mockClear();
    renderAt("/blog/nope");
    await screen.findByRole("heading", { level: 1, name: "Post not found" });
    expect(track).not.toHaveBeenCalled();
  });

  it("renders no blocks the posts carry no data for (takeaways, CTA, FAQ, author bio)", async () => {
    renderAt("/blog/explore-from-your-browser");
    const article = (await screen.findByRole("heading", { level: 1 })).closest("article")!;
    for (const name of [/takeaway/i, /faq|frequently asked/i, /about the author/i, /get started|sign up|download/i]) {
      expect(within(article).queryByRole("heading", { name })).toBeNull();
    }
    expect(within(article).queryByRole("button", { name: /get started|sign up|download/i })).toBeNull();
    expect(article.querySelector("details, dl")).toBeNull();
  });
});
