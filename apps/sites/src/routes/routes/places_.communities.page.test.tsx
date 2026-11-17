// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps, ReactNode } from "react";
import { createRoutesStub, useLocation } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import { fetchCommunitiesPage, type BrowseCommunity } from "@/data/lib/catalyst/social/communities-list";

import PlacesCommunities from "./places_.communities";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);
vi.mock("@ui/web/frames/SitesChrome", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("@/data/lib/catalyst/social/communities-list", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/communities-list")>()),
  fetchCommunitiesPage: vi.fn(),
}));

const community = (id: string, overrides: Partial<BrowseCommunity> = {}): BrowseCommunity => ({
  id,
  name: `Community ${id}`,
  description: null,
  ownerName: null,
  privacy: "public",
  membersCount: 1,
  thumbnailUrl: null,
  ...overrides,
});

/** Every read answers this page, or fails with this error. */
function serve(page: BrowseCommunity[] | Error, total = Array.isArray(page) ? page.length : 0) {
  vi.mocked(fetchCommunitiesPage).mockImplementation(async () => {
    if (page instanceof Error) throw page;
    return { rows: page, received: page.length, total };
  });
}

const fullPage = (prefix: string) => Array.from({ length: 24 }, (_, i) => community(`${prefix}${i}`));

function Location() {
  const location = useLocation();
  return <output data-testid="location">{location.pathname + location.search}</output>;
}

function renderPage(url = "/places/communities", unavailable = false) {
  const Stub = createRoutesStub([
    {
      path: "/places/communities",
      Component: (props: ComponentProps<typeof PlacesCommunities>) => (
        <>
          <PlacesCommunities {...props} />
          <Location />
        </>
      ),
      loader: () => ({ sid: "sid", dehydratedState: undefined, unavailable }),
    },
    { path: "/places", Component: () => <p>Places page</p> },
    { path: "/landings/community-detail", Component: () => <p>Community page</p> },
  ]);
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <Stub initialEntries={[url]} />
    </QueryClientProvider>,
  );
  return { user: userEvent.setup() };
}

const location = () => screen.getByTestId("location").textContent ?? "";

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("when the communities load", () => {
  it("should list each community as a card linking to its page", async () => {
    serve([community("a", { name: "Builders", membersCount: 1340, ownerName: "Jane", privacy: "private" })]);
    renderPage();
    await screen.findByRole("link", { name: "Builders" });
    const grid = screen.getByRole("list", { name: "Communities" });
    const link = within(grid).getByRole("link", { name: "Builders" });
    expect(link.getAttribute("href")).toBe("/landings/community-detail?id=a");
    expect(within(grid).getByText("1,340 members")).toBeTruthy();
    expect(within(grid).getByText("Jane")).toBeTruthy();
    expect(within(grid).getByText("Private")).toBeTruthy();
  });

  it("should draw the cover the row names, and request no image when it names none", async () => {
    serve([community("a", { name: "Mirrored", thumbnailUrl: "https://cdn.example/a.png" }), community("b", { name: "Local" })]);
    renderPage();
    await screen.findByRole("link", { name: "Mirrored" });
    const cards = [...document.querySelectorAll("article")];
    expect(cards).toHaveLength(2);
    expect(cards[0].querySelector("img")?.getAttribute("src")).toBe("https://cdn.example/a.png");
    expect(cards[1].querySelector("img")).toBeNull();
  });

  it("should name a single member in the singular", async () => {
    serve([community("a", { membersCount: 1 })]);
    renderPage();
    expect(await screen.findByText("1 member")).toBeTruthy();
  });

  it("should show the tabs with Communities selected and Places linking to /places", async () => {
    serve([community("a")]);
    renderPage();
    const tabs = await screen.findByRole("tablist", { name: "Places sections" });
    expect(within(tabs).getByRole("tab", { name: "Communities" }).getAttribute("aria-selected")).toBe("true");
    expect(within(tabs).getByRole("tab", { name: "Places" }).getAttribute("href")).toBe("/places");
  });

  it("should show loading placeholders until the first page arrives", async () => {
    vi.mocked(fetchCommunitiesPage).mockImplementation(() => new Promise(() => {}));
    renderPage();
    expect((await screen.findAllByRole("progressbar", { name: "Loading communities…" })).length).toBe(4);
    expect(screen.getByRole("list", { name: "Communities" }).getAttribute("aria-busy")).toBe("true");
  });

  it("should say so when there are no communities", async () => {
    serve([]);
    renderPage();
    // Shown, and announced through the page status.
    expect(await screen.findAllByText("No communities found.")).toHaveLength(2);
    expect(screen.queryByRole("list", { name: "Communities" })).toBeNull();
  });
});

describe("when the list fails", () => {
  it("should show the error with Retry and recover when the list answers again", async () => {
    serve(new Error("Catalyst returned 502"));
    const { user } = renderPage();
    expect((await screen.findByRole("alert")).textContent).toBe("Something went wrong loading communities.");
    expect(screen.queryByText("No communities found.")).toBeNull();
    serve([community("back", { name: "Back" })]);
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("link", { name: "Back" })).toBeTruthy();
  });

  it("should show the error of a 503 document without reading again until Retry", async () => {
    serve([community("back", { name: "Back" })]);
    const { user } = renderPage("/places/communities", true);
    expect((await screen.findByRole("alert")).textContent).toBe("Something went wrong loading communities.");
    expect(fetchCommunitiesPage).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("link", { name: "Back" })).toBeTruthy();
  });
});

describe("when the visitor searches", () => {
  it("should put the search in ?search once typing pauses and read the list with it", async () => {
    serve([community("a")]);
    const { user } = renderPage();
    await user.type(await screen.findByRole("textbox", { name: "Search communities" }), "guild");
    await waitFor(() => expect(location()).toBe("/places/communities?search=guild"));
    await waitFor(() => expect(fetchCommunitiesPage).toHaveBeenCalledWith({ offset: 0, search: "guild" }, expect.anything()));
  });

  it("should not send a search shorter than three characters", async () => {
    serve([community("a")]);
    const { user } = renderPage();
    await user.type(await screen.findByRole("textbox", { name: "Search communities" }), "gu");
    await waitFor(() => expect(location()).toBe("/places/communities?search=gu"));
    expect(fetchCommunitiesPage).not.toHaveBeenCalledWith(expect.objectContaining({ search: "gu" }), expect.anything());
    expect(vi.mocked(fetchCommunitiesPage).mock.calls.every(([params]) => params.search === "")).toBe(true);
  });

  it("should clear the search from the field's clear button", async () => {
    serve([community("a")]);
    const { user } = renderPage("/places/communities?search=guild");
    expect((await screen.findByRole("textbox", { name: "Search communities" })).getAttribute("value")).toBe("guild");
    await user.click(screen.getByRole("button", { name: "Clear search" }));
    await waitFor(() => expect(location()).toBe("/places/communities"));
  });
});

describe("when the URL's search changes from history", () => {
  it("should not send a search of only spaces", async () => {
    serve([community("a")]);
    renderPage("/places/communities?search=%20%20%20%20");
    await screen.findByRole("link", { name: "Community a" });
    expect(vi.mocked(fetchCommunitiesPage).mock.calls.every(([params]) => params.search === "")).toBe(true);
  });
});

describe("when there are more communities than a page", () => {
  it("should load the next page from the offset after the first and append it", async () => {
    serve(fullPage("p"), 30);
    const { user } = renderPage();
    const more = await screen.findByRole("button", { name: "Load more communities" });
    serve([community("p23"), community("next", { name: "Next one" })], 30);
    await user.click(more);
    expect(await screen.findByRole("link", { name: "Next one" })).toBeTruthy();
    expect(fetchCommunitiesPage).toHaveBeenLastCalledWith({ offset: 24, search: "" }, expect.anything());
    expect(screen.getAllByRole("link", { name: "Community p23" })).toHaveLength(1);
    expect(screen.queryByRole("button", { name: "Load more communities" })).toBeNull();
  });

  it("should keep the loaded cards and offer Retry when the next page fails", async () => {
    serve(fullPage("p"), 30);
    const { user } = renderPage();
    const more = await screen.findByRole("button", { name: "Load more communities" });
    serve(new Error("timeout"));
    await user.click(more);
    expect((await screen.findByRole("alert")).textContent).toBe("Something went wrong loading communities.");
    expect(screen.getByRole("link", { name: "Community p0" })).toBeTruthy();
    serve([community("next", { name: "Next one" })], 30);
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("link", { name: "Next one" })).toBeTruthy();
  });

  it("should say nothing matched yet and still offer the next page when a whole first page is unusable", async () => {
    vi.mocked(fetchCommunitiesPage).mockResolvedValue({ rows: [], received: 24, total: 30 });
    renderPage();
    expect(await screen.findByRole("button", { name: "Load more communities" })).toBeTruthy();
    expect(screen.getAllByText("No communities found.")).toHaveLength(2);
    expect(screen.getByRole("button", { name: "Load more communities" })).toBeTruthy();
    expect(screen.queryByRole("list", { name: "Communities" })).toBeNull();
  });

  it("should announce how many communities are listed", async () => {
    serve(fullPage("p"), 30);
    renderPage();
    await screen.findByRole("link", { name: "Community p0" });
    expect(screen.getAllByRole("status").map((node) => node.textContent)).toContain("24+ communities");
  });

  it("should offer no next page after the last one", async () => {
    serve([community("only")], 1);
    renderPage();
    await screen.findByRole("link", { name: "Community only" });
    expect(screen.queryByRole("button", { name: "Load more communities" })).toBeNull();
  });
});
