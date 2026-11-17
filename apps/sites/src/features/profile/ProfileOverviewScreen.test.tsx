// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const net = vi.hoisted(() => ({ fetchUserBadges: vi.fn() }));
vi.mock("@/data/lib/catalyst/social/badges", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/badges")>()),
  fetchUserBadges: net.fetchUserBadges,
}));

import { emptyProfile, type ProfileVM } from "@/data/lib/catalyst/social/profile";

import { ProfileOverviewScreen } from "./ProfileOverviewScreen";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const tabs = [
  { id: "overview", label: "Overview", href: `/profile/${A}` },
  { id: "places", label: "Places", href: `/profile/${A}/places` },
];

function profile(over: Partial<ProfileVM> = {}): ProfileVM {
  return { ...emptyProfile(A), name: "Alice", nameColor: "#ff8362", ...over };
}

let client: QueryClient;

/** The copy button's own live region, next to it in the header. */
const copyStatus = () =>
  within(screen.getByRole("button", { name: "Copy address" }).parentElement as HTMLElement).getByRole("status");

function show(vm: ProfileVM, own = false, onTab = vi.fn(), hasProfile = false) {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(
    [
      {
        path: "*",
        element: <ProfileOverviewScreen profile={vm} hasProfile={hasProfile} own={own} tabs={tabs} onTab={onTab} />,
      },
    ],
    { initialEntries: [`/profile/${A}`] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return router;
}

const realClipboard = Object.getOwnPropertyDescriptor(navigator, "clipboard");
function stubClipboard(value: unknown) {
  Object.defineProperty(navigator, "clipboard", { value, configurable: true });
}

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  if (realClipboard) Object.defineProperty(navigator, "clipboard", realClipboard);
  else delete (navigator as { clipboard?: unknown }).clipboard;
});

describe("profile overview screen, header", () => {
  it("shows a claimed name with the verified mark and no number suffix", () => {
    show(profile({ hasClaimedName: true }));

    expect(screen.getByRole("heading", { level: 1, name: "Alice" })).toBeTruthy();
    expect(screen.getByRole("img", { name: "Verified" })).toBeTruthy();
    expect(screen.queryByText("#f295")).toBeNull();
  });

  it("shows an unclaimed name with the address's last four characters", () => {
    show(profile({ hasClaimedName: false }));

    expect(screen.getByText("#f295")).toBeTruthy();
    expect(screen.queryByLabelText("Verified")).toBeNull();
    expect(screen.getByText("0x4e9c…f295")).toBeTruthy();
  });

  it("copies the full address and says so", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    stubClipboard({ writeText });
    show(profile());
    const status = copyStatus();
    expect(status.textContent).toBe("");

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Copy address" }));
    });

    expect(writeText).toHaveBeenCalledWith(A);
    expect(copyStatus()).toBe(status);
    expect(status.textContent).toContain("Address copied!");
  });

  describe.each([true, false])("when copying the same address twice and clipboard success is %s", (succeeds) => {
    let first: ChildNode | null;
    beforeEach(async () => {
      stubClipboard({ writeText: succeeds
        ? vi.fn().mockResolvedValueOnce(undefined).mockResolvedValueOnce(undefined)
        : vi.fn().mockRejectedValueOnce(new Error("denied")).mockRejectedValueOnce(new Error("denied")) });
      show(profile());
      await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Copy address" })); });
      first = copyStatus().firstChild;
      await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Copy address" })); });
    });
    it("should update the live region contents for the repeated result", () => {
      expect(copyStatus().firstChild).not.toBe(first);
    });
  });

  it("says so when the clipboard refuses the address, or there is none", async () => {
    stubClipboard({ writeText: vi.fn().mockRejectedValue(new Error("denied")) });
    show(profile());

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Copy address" }));
    });
    expect(copyStatus().textContent).toBe("Couldn't copy the address");
    cleanup();

    stubClipboard(undefined);
    show(profile());
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Copy address" }));
    });
    expect(copyStatus().textContent).toBe("Couldn't copy the address");
  });

  it("offers the owner a NAME, or the worlds once they have one, and a visitor neither", () => {
    show(profile({ hasClaimedName: false }), true);
    expect(screen.getByRole("link", { name: "Get a Unique Name" }).getAttribute("href")).toBe("/shop/names");
    cleanup();

    show(profile({ hasClaimedName: true }), true);
    expect(screen.getByRole("link", { name: "Manage World" }).getAttribute("href")).toBe("/create/worlds");
    cleanup();

    show(profile(), false);
    expect(screen.queryByRole("link", { name: /Unique Name|Manage World/ })).toBeNull();
  });
});

describe("profile overview screen, tabs", () => {
  it("does not count a modifier or middle click, which opens a new browser tab, as a tab change", () => {
    const onTab = vi.fn();
    show(profile(), false, onTab);
    const places = within(screen.getByRole("tablist", { name: "Profile sections" })).getByRole("tab", { name: "Places" });

    fireEvent.click(places, { metaKey: true });
    fireEvent.click(places, { ctrlKey: true });
    fireEvent.click(places, { button: 1 });

    expect(onTab).not.toHaveBeenCalled();
  });

  it("selects Overview, labels its panel with it and navigates in the app on a tab", async () => {
    const onTab = vi.fn();
    const router = show(profile(), false, onTab);
    const bar = screen.getByRole("tablist", { name: "Profile sections" });
    const overview = within(bar).getByRole("tab", { name: "Overview" });
    const places = within(bar).getByRole("tab", { name: "Places" });

    expect(overview.getAttribute("aria-selected")).toBe("true");
    expect(places.getAttribute("aria-selected")).toBe("false");
    expect(places.getAttribute("href")).toBe(`/profile/${A}/places`);
    const panel = screen.getByRole("tabpanel", { name: "Overview" });
    expect(overview.getAttribute("aria-controls")).toBe(panel.id);
    expect(within(panel).getByRole("heading", { name: "About" })).toBeTruthy();
    await act(async () => {
      fireEvent.click(places);
    });

    expect(onTab).toHaveBeenCalledWith("places");
    expect(router.state.location.pathname).toBe(`/profile/${A}/places`);
  });
});

describe("profile overview screen, About", () => {
  it("shows the bio, or the visitor's and the owner's empty text", () => {
    show(profile({ bio: "Builder of worlds" }));
    expect(screen.getByText("Builder of worlds")).toBeTruthy();
    cleanup();

    show(profile({ bio: "  " }));
    expect(screen.getByText("This member has not added a bio yet.")).toBeTruthy();
    cleanup();

    show(profile({ bio: "" }), true);
    expect(screen.getByText("No information yet. Jump in world and complete your bio!")).toBeTruthy();
  });

  it("lists the filled details with their labels", () => {
    show(profile({ info: [{ key: "country", label: "Country", value: "Argentina", icon: "globe" }] }));

    expect(screen.getByText("Country")).toBeTruthy();
    expect(screen.getByText("Argentina")).toBeTruthy();
  });

  it("renders the links it is given, refusing any non-http(s) one, even twice the same", () => {
    const errors = vi.spyOn(console, "error").mockImplementation(() => {});
    show(
      profile({
        links: [
          { title: "My blog", url: "https://blog.example.com/" },
          { title: "My blog", url: "https://blog.example.com/" },
          { title: "click", url: "javascript:alert(1)" },
        ],
      }),
    );

    const links = within(screen.getByRole("heading", { name: "Links" }).parentElement as HTMLElement).getAllByRole("link");
    expect(links.map((link) => [link.textContent, link.getAttribute("href"), link.getAttribute("rel")])).toEqual([
      ["My blog", "https://blog.example.com/", "noopener noreferrer"],
      ["My blog", "https://blog.example.com/", "noopener noreferrer"],
    ]);
    expect(errors.mock.calls.some(([message]) => String(message).includes("same key"))).toBe(false);
    errors.mockRestore();
  });

  it("leaves the Links section out when no link is safe", () => {
    show(profile({ links: [{ title: "x", url: "data:text/html,hi" }] }));

    expect(screen.queryByRole("heading", { name: "Links" })).toBeNull();
  });

  it("shows the initial for a face image already failed when the component mounts (as before hydration)", () => {
    const proto = HTMLImageElement.prototype;
    const complete = Object.getOwnPropertyDescriptor(proto, "complete");
    const width = Object.getOwnPropertyDescriptor(proto, "naturalWidth");
    Object.defineProperty(proto, "complete", { configurable: true, get: () => true });
    Object.defineProperty(proto, "naturalWidth", { configurable: true, get: () => 0 });
    try {
      show(profile({ avatarUrl: "https://example.com/face.png" }));

      expect(screen.queryByAltText("Alice")).toBeNull();
      expect(screen.getByText("A")).toBeTruthy();
    } finally {
      if (complete) Object.defineProperty(proto, "complete", complete);
      if (width) Object.defineProperty(proto, "naturalWidth", width);
    }
  });

  it("shows the full-body snapshot in the avatar column when there is one", () => {
    show(profile({ bodyUrl: "https://example.com/body.png" }));

    expect(screen.getByAltText("Alice's avatar").getAttribute("src")).toBe("https://example.com/body.png");
  });
});

describe("profile overview screen, badges", () => {
  it("does not read badges for a wallet with no profile, and says it has none", () => {
    show(profile(), false, vi.fn(), false);

    expect(net.fetchUserBadges).not.toHaveBeenCalled();
    expect(screen.getByText("No badges yet. Jump in world and start collecting badges!")).toBeTruthy();
  });

  it("shows each badge with its name, tier, blurb and unlock date", async () => {
    net.fetchUserBadges.mockResolvedValue([
      {
        id: "explorer",
        name: "Explorer",
        imageUrl: "https://example.com/explorer.png",
        tierName: "Gold",
        description: "Visit 100 scenes",
        unlockedAt: Date.UTC(2026, 2, 3),
      },
    ]);

    show(profile(), false, vi.fn(), true);

    expect(await screen.findByAltText("Explorer")).toBeTruthy();
    const tip = screen.getByRole("tooltip", { hidden: true });
    expect(tip.textContent).toContain("Gold");
    expect(tip.textContent).toContain("Visit 100 scenes");
    expect(tip.textContent).toContain("Unlocked on Mar 3, 2026");
    expect(net.fetchUserBadges).toHaveBeenCalledWith(A, { signal: expect.any(AbortSignal) });
  });

  it("announces how the badges loading ended in the same live region", async () => {
    net.fetchUserBadges.mockResolvedValue([]);

    show(profile(), false, vi.fn(), true);
    const region = screen.getAllByRole("status").find((el) => el.textContent === "Loading badges…");
    expect(region).toBeTruthy();

    await waitFor(() => expect(region?.textContent).toBe("No badges yet. Jump in world and start collecting badges!"));
    expect(region?.isConnected).toBe(true);
    expect(region?.getAttribute("role")).toBe("status");
  });

  describe.each([[1, "1 badge"], [2, "2 badges"]] as const)("when %s badges finish loading", (count, label) => {
    beforeEach(() => {
      net.fetchUserBadges.mockResolvedValueOnce(Array.from({ length: count }, (_, i) => ({ id: `badge-${i}`, name: "Explorer" })));
      show(profile(), false, vi.fn(), true);
    });
    it("should keep the loaded count in an accessible status region", async () => {
      await waitFor(() => expect(screen.getAllByRole("status").some(region => region.textContent === label)).toBe(true));
    });
  });

  it("keeps the badges on screen when a background re-read fails", async () => {
    net.fetchUserBadges.mockResolvedValueOnce([{ id: "explorer", name: "Explorer", imageUrl: "https://example.com/e.png" }]);
    show(profile(), false, vi.fn(), true);
    await screen.findByAltText("Explorer");
    net.fetchUserBadges.mockRejectedValue(new Error("offline"));

    await act(async () => {
      await client.refetchQueries({ queryKey: ["profile", "badges"] });
    });

    expect(client.getQueryCache().findAll({ queryKey: ["profile", "badges"] })[0]?.state.status).toBe("error");
    await waitFor(() => expect(screen.getByAltText("Explorer")).toBeTruthy());
    expect(screen.queryByText("Badges are unavailable right now.")).toBeNull();
  });

  it("says the badges are unavailable when they cannot be read, not that there are none", async () => {
    net.fetchUserBadges.mockRejectedValue(new Error("offline"));

    show(profile(), false, vi.fn(), true);

    await waitFor(() => expect(screen.getByText("Badges are unavailable right now.")).toBeTruthy());
    expect(screen.queryByText("No badges yet. Jump in world and start collecting badges!")).toBeNull();
  });
});
