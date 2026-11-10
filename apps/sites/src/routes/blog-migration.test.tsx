// @vitest-environment jsdom
import { track } from "@/core/lib/telemetry/track";
import "@/features/locale/test-setup";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import { Link, Outlet, createRoutesStub } from "react-router";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createFixtureBlogSource } from "@/data/lib/blog/fixture-source";
import { currentBlogSource } from "@/data/lib/blog/current-source";
import { unboundBlogSource, type BlogSource } from "@/data/lib/blog/source";
import { blogKeys } from "@/data/lib/blog/keys";
import { QUERY_CLIENT_CONFIG } from "@/core/lib/query/defaults";
import {
  homeLoader,
  homeClientLoader,
  BlogHome,
  type HomeData,
} from "./lib/blog-home";
import { categoryLoader } from "./lib/blog-category";
import { BlogErrorBoundary } from "./lib/blog-page";
import PostRoute, {
  clientLoader as postClientLoader,
} from "./routes/blog.$categorySlug.$postSlug";
import { clientLoader as searchClientLoader } from "./routes/blog.search";

vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/data/lib/blog/current-source", () => ({
  currentBlogSource: vi.fn(),
}));
function payload(value: unknown): HomeData {
  return (value as { data: HomeData }).data;
}

describe("when blog routes use the host cache and document handoff", () => {
  afterEach(() => vi.restoreAllMocks());
  let source: BlogSource;
  let request: Request;
  let host: QueryClient;
  beforeEach(() => {
    vi.mocked(track).mockClear();
    source = createFixtureBlogSource();
    vi.spyOn(source, "listPosts");
    vi.mocked(currentBlogSource).mockResolvedValue(source);
    request = new Request("https://sites.example/blog");
    host = new QueryClient(QUERY_CLIENT_CONFIG);
  });
  it("should hydrate the first page into the host without a duplicate source fetch", async () => {
    const data = payload(await homeLoader({ request }));
    const Routes = createRoutesStub([
      {
        id: "root",
        path: "/",
        loader: () => ({ sid: "test-sid", locale: "en" }),
        Component: () => (
          <QueryClientProvider client={host}>
            <Outlet />
          </QueryClientProvider>
        ),
        children: [
          {
            path: "blog",
            loader: () => data,
            Component: () => <BlogHome data={data} />,
          },
          {
            path: "blog/:categorySlug/:postSlug",
            loader: () => null,
            Component: () => (
              <PostRoute
                {...({ loaderData: null } as Parameters<typeof PostRoute>[0])}
              />
            ),
          },
        ],
      },
    ]);
    render(<Routes initialEntries={["/blog"]} />);
    await waitFor(() =>
      expect(
        screen
          .getAllByRole("link")
          .some((link) =>
            link.getAttribute("href")?.includes("/blog/announcements/"),
          ),
      ).toBe(true),
    );
    expect(document.title).toBe("Decentraland Blog | Decentraland");
    expect(
      document.head.querySelectorAll('link[rel="canonical"]'),
    ).toHaveLength(1);
    expect(source.listPosts).toHaveBeenCalledTimes(1);
    expect(track).toHaveBeenCalledWith(
      "lp_blog_viewed",
      { post_count: expect.any(Number) },
      { sid: "test-sid", story: "misc/blog" },
    );
    expect(host.getQueryData(blogKeys.list({ scope: "home" }))).toBeDefined();
    const link = screen
      .getAllByRole("link")
      .find(
        (link) =>
          link.getAttribute("href") ===
          "/blog/announcements/decentralands-2024-recap",
      )!;
    await userEvent.click(link);
    await waitFor(() =>
      expect(document.title).toBe("Decentraland’s 2024 Recap | Decentraland"),
    );
    expect(document.head.querySelectorAll("title")).toHaveLength(1);
    expect(
      document.head.querySelectorAll('meta[name="description"]'),
    ).toHaveLength(1);
    expect(
      document.head.querySelector('link[rel="canonical"]'),
    ).toHaveAttribute(
      "href",
      expect.stringContaining("/blog/announcements/decentralands-2024-recap"),
    );
    expect(track).toHaveBeenCalledWith(
      "lp_blog_post_clicked",
      { slug: "decentralands-2024-recap" },
      { sid: "test-sid", story: "misc/blog" },
    );

    host.clear();
  });
  it("should let navigation query the host rather than calling a server loader", () => {
    expect([
      homeClientLoader(),
      postClientLoader(),
      searchClientLoader(),
    ]).toEqual([null, null, null]);
  });
  it("should keep separate server requests independent and dehydrate only one list page", async () => {
    const [first, second] = await Promise.all([
      homeLoader({ request }),
      homeLoader({ request: new Request("https://sites.example/blog") }),
    ]);
    const a = payload(first).dehydratedState;
    const b = payload(second).dehydratedState;
    expect(a).not.toBe(b);
    const list = a.queries.find(
      (query) =>
        JSON.stringify(query.queryKey) ===
        JSON.stringify(blogKeys.list({ scope: "home" })),
    );
    expect((list?.state.data as { pages: unknown[] }).pages).toHaveLength(1);
  });
  it("should return the honest unavailable response when the production source is unconfigured", async () => {
    vi.mocked(currentBlogSource).mockResolvedValue(unboundBlogSource);
    await expect(homeLoader({ request })).rejects.toMatchObject({
      status: 503,
    });
  });
  it.each(["navigate", "retry"] as const)(
    "should recover by %s after a missing article on a client navigation without retaining its failed query",
    async (action) => {
      const user = userEvent.setup();
      const realPost = source.getPost.bind(source);
      vi.spyOn(source, "getPost")
        .mockResolvedValueOnce(null)
        .mockImplementation(realPost);
      vi.spyOn(console, "error").mockImplementation(() => undefined);
      host = new QueryClient({
        ...QUERY_CLIENT_CONFIG,
        defaultOptions: { queries: { retry: false } },
      });
      const href = "/blog/announcements/decentralands-2024-recap";
      const Routes = createRoutesStub([
        {
          id: "root",
          path: "/",
          Component: () => (
            <QueryClientProvider client={host}>
              <Outlet />
            </QueryClientProvider>
          ),
          children: [
            {
              path: "blog",
              Component: () => <Link to={href}>Read the recap</Link>,
            },
            {
              path: "blog/:categorySlug/:postSlug",
              loader: () => null,
              Component: () => (
                <PostRoute
                  {...({ loaderData: null } as Parameters<typeof PostRoute>[0])}
                />
              ),
              ErrorBoundary: BlogErrorBoundary,
            },
          ],
        },
      ]);
      render(<Routes initialEntries={["/blog"]} />);
      await user.click(screen.getByRole("link", { name: "Read the recap" }));
      expect(await screen.findByRole("alert")).toHaveTextContent(
        "Failed to load post",
      );
      if (action === "navigate") {
        await user.click(
          screen.getByRole("link", { name: "Back to the blog" }),
        );
        await user.click(
          await screen.findByRole("link", { name: "Read the recap" }),
        );
      } else {
        await user.click(screen.getByRole("button", { name: "Try again" }));
      }
      expect(
        await screen.findByRole("heading", {
          level: 1,
          name: "Decentraland’s 2024 Recap",
        }),
      ).toBeInTheDocument();
      expect(source.getPost).toHaveBeenCalledTimes(2);
      expect(
        vi
          .mocked(track)
          .mock.calls.filter(([event]) => event === "lp_blog_post_viewed"),
      ).toHaveLength(1);
      host.clear();
    },
  );

  it("should return 404 for an unknown category", async () => {
    await expect(
      categoryLoader({ request, params: { slug: "missing-category" } }),
    ).rejects.toMatchObject({ status: 404 });
  });
});
