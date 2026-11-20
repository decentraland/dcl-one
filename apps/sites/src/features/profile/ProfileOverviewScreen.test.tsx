import { ChromeNavContext } from "@dcl/one-ui/components/AppChrome/chrome-nav";
// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, type Mock, type MockInstance, vi } from "vitest";

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
      <ChromeNavContext.Provider value={{ headerLayout: "native" }}>
        <RouterProvider router={router} />
      </ChromeNavContext.Provider>
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
  vi.restoreAllMocks();
  vi.resetAllMocks();
  if (realClipboard) Object.defineProperty(navigator, "clipboard", realClipboard);
  else delete (navigator as { clipboard?: unknown }).clipboard;
});

describe("ProfileOverviewScreen header", () => {
  describe("when the name is claimed", () => {
    beforeEach(() => {
      show(profile({ hasClaimedName: true }));
    });

    it("should show the name as the page heading", () => {
      expect(screen.getByRole("heading", { level: 1, name: "Alice" })).toBeTruthy();
    });

    it("should show the verified mark", () => {
      expect(screen.getByRole("img", { name: "Verified" })).toBeTruthy();
    });

    it("should show no number suffix", () => {
      expect(screen.queryByText("#f295")).toBeNull();
    });
  });

  describe("when the page renders", () => {
    beforeEach(() => {
      show(profile());
    });

    it("should keep the global header in the layout flow above Profile", () => {
      const layout = screen.getByRole("heading", { level: 1, name: "Alice" }).closest("[data-site-layout]");
      expect(layout?.getAttribute("data-header-layout")).toBe("native");
      expect(layout?.hasAttribute("data-header-overlay")).toBe(false);
    });
  });

  describe("when the name is not claimed", () => {
    beforeEach(() => {
      show(profile({ hasClaimedName: false }));
    });

    it("should show the address's last four characters as a suffix", () => {
      expect(screen.getByText("#f295")).toBeTruthy();
    });

    it("should show no verified mark", () => {
      expect(screen.queryByLabelText("Verified")).toBeNull();
    });

    it("should show the short address", () => {
      expect(screen.getByText("0x4e9c…f295")).toBeTruthy();
    });
  });

  describe("when the address is copied", () => {
    let writeText: Mock;
    let status: HTMLElement;

    beforeEach(() => {
      writeText = vi.fn().mockResolvedValueOnce(undefined);
      stubClipboard({ writeText });
      show(profile());
      status = copyStatus();
    });

    it("should start with an empty status", () => {
      expect(status.textContent).toBe("");
    });

    describe("and the clipboard accepts it", () => {
      beforeEach(async () => {
        await act(async () => {
          fireEvent.click(screen.getByRole("button", { name: "Copy address" }));
        });
      });

      it("should copy the full address", () => {
        expect(writeText).toHaveBeenCalledWith(A);
      });

      it("should say so in the same live region", () => {
        expect(copyStatus()).toBe(status);
        expect(status.textContent).toContain("Address copied!");
      });
    });
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

  describe("when the clipboard refuses the address", () => {
    beforeEach(async () => {
      stubClipboard({ writeText: vi.fn().mockRejectedValueOnce(new Error("denied")) });
      show(profile());
      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Copy address" }));
      });
    });

    it("should say the address could not be copied", () => {
      expect(copyStatus().textContent).toBe("Couldn't copy the address");
    });
  });

  describe("when there is no clipboard", () => {
    beforeEach(async () => {
      stubClipboard(undefined);
      show(profile());
      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Copy address" }));
      });
    });

    it("should say the address could not be copied", () => {
      expect(copyStatus().textContent).toBe("Couldn't copy the address");
    });
  });

  describe("when the owner has no NAME", () => {
    beforeEach(() => {
      show(profile({ hasClaimedName: false }), true);
    });

    it("should offer a NAME", () => {
      expect(screen.getByRole("link", { name: "Get a Unique Name" }).getAttribute("href")).toBe("/shop/names");
    });
  });

  describe("when the owner has a NAME", () => {
    beforeEach(() => {
      show(profile({ hasClaimedName: true }), true);
    });

    it("should offer the worlds", () => {
      expect(screen.getByRole("link", { name: "Manage World" }).getAttribute("href")).toBe("/download/creator-hub");
    });
  });

  describe("when a visitor reads the profile", () => {
    beforeEach(() => {
      show(profile(), false);
    });

    it("should offer neither a NAME nor the worlds", () => {
      expect(screen.queryByRole("link", { name: /Unique Name|Manage World/ })).toBeNull();
    });
  });
});

describe("ProfileOverviewScreen tabs", () => {
  let onTab: Mock;
  let router: ReturnType<typeof show>;
  let places: HTMLElement;

  beforeEach(() => {
    onTab = vi.fn();
    router = show(profile(), false, onTab);
    places = within(screen.getByRole("tablist", { name: "Profile sections" })).getByRole("tab", { name: "Places" });
  });

  describe("when a tab is clicked with a modifier or the middle button, which opens a new browser tab", () => {
    beforeEach(() => {
      fireEvent.click(places, { metaKey: true });
      fireEvent.click(places, { ctrlKey: true });
      fireEvent.click(places, { button: 1 });
    });

    it("should not count it as a tab change", () => {
      expect(onTab).not.toHaveBeenCalled();
    });
  });

  describe("when the page opens", () => {
    let overview: HTMLElement;

    beforeEach(() => {
      overview = within(screen.getByRole("tablist", { name: "Profile sections" })).getByRole("tab", { name: "Overview" });
    });

    it("should select Overview", () => {
      expect(overview.getAttribute("aria-selected")).toBe("true");
      expect(places.getAttribute("aria-selected")).toBe("false");
    });

    it("should link each tab to its path", () => {
      expect(places.getAttribute("href")).toBe(`/profile/${A}/places`);
    });

    it("should label the Overview panel with its tab", () => {
      const panel = screen.getByRole("tabpanel", { name: "Overview" });
      expect(overview.getAttribute("aria-controls")).toBe(panel.id);
      expect(within(panel).getByRole("heading", { name: "About" })).toBeTruthy();
    });
  });

  describe("when a tab is clicked", () => {
    beforeEach(async () => {
      await act(async () => {
        fireEvent.click(places);
      });
    });

    it("should report the tab change", () => {
      expect(onTab).toHaveBeenCalledWith("places");
    });

    it("should navigate in the app", () => {
      expect(router.state.location.pathname).toBe(`/profile/${A}/places`);
    });
  });
});

describe("ProfileOverviewScreen About", () => {
  describe("when the member has a bio", () => {
    beforeEach(() => {
      show(profile({ bio: "Builder of worlds" }));
    });

    it("should show the bio", () => {
      expect(screen.getByText("Builder of worlds")).toBeTruthy();
    });
  });

  describe("when the bio is blank", () => {
    describe("and a visitor reads the profile", () => {
      beforeEach(() => {
        show(profile({ bio: "  " }));
      });

      it("should show the visitor's empty text", () => {
        expect(screen.getByText("This member has not added a bio yet.")).toBeTruthy();
      });
    });

    describe("and the owner reads the profile", () => {
      beforeEach(() => {
        show(profile({ bio: "" }), true);
      });

      it("should show the owner's empty text", () => {
        expect(screen.getByText("No information yet. Jump in world and complete your bio!")).toBeTruthy();
      });
    });
  });

  describe("when the member filled a detail", () => {
    beforeEach(() => {
      show(profile({ info: [{ key: "country", label: "Country", value: "Argentina", icon: "globe" }] }));
    });

    it("should list the detail with its label", () => {
      expect(screen.getByText("Country")).toBeTruthy();
      expect(screen.getByText("Argentina")).toBeTruthy();
    });
  });

  describe("when the links include a repeated one and a non-http(s) one", () => {
    let errors: MockInstance<typeof console.error>;

    beforeEach(() => {
      errors = vi.spyOn(console, "error").mockImplementation(() => {});
      show(
        profile({
          links: [
            { title: "My blog", url: "https://blog.example.com/" },
            { title: "My blog", url: "https://blog.example.com/" },
            { title: "click", url: "javascript:alert(1)" },
          ],
        }),
      );
    });

    it("should render the http(s) links, even twice the same, and refuse the other", () => {
      const links = within(screen.getByRole("heading", { name: "Links" }).parentElement as HTMLElement).getAllByRole("link");
      expect(links.map((link) => [link.textContent, link.getAttribute("href"), link.getAttribute("rel")])).toEqual([
        ["My blog", "https://blog.example.com/", "noopener noreferrer"],
        ["My blog", "https://blog.example.com/", "noopener noreferrer"],
      ]);
    });

    it("should not warn about a repeated key", () => {
      expect(errors.mock.calls.some(([message]) => String(message).includes("same key"))).toBe(false);
    });
  });

  describe("when no link is safe", () => {
    beforeEach(() => {
      show(profile({ links: [{ title: "x", url: "data:text/html,hi" }] }));
    });

    it("should leave the Links section out", () => {
      expect(screen.queryByRole("heading", { name: "Links" })).toBeNull();
    });
  });

  describe("when the face image already failed when the component mounts (as before hydration)", () => {
    let complete: PropertyDescriptor | undefined;
    let width: PropertyDescriptor | undefined;

    beforeEach(() => {
      const proto = HTMLImageElement.prototype;
      complete = Object.getOwnPropertyDescriptor(proto, "complete");
      width = Object.getOwnPropertyDescriptor(proto, "naturalWidth");
      Object.defineProperty(proto, "complete", { configurable: true, get: () => true });
      Object.defineProperty(proto, "naturalWidth", { configurable: true, get: () => 0 });
      show(profile({ avatarUrl: "https://example.com/face.png" }));
    });

    afterEach(() => {
      const proto = HTMLImageElement.prototype;
      if (complete) Object.defineProperty(proto, "complete", complete);
      if (width) Object.defineProperty(proto, "naturalWidth", width);
    });

    it("should drop the image", () => {
      expect(screen.queryByAltText("Alice")).toBeNull();
    });

    it("should show the initial", () => {
      expect(screen.getByText("A")).toBeTruthy();
    });
  });

  describe("when there is a full-body snapshot", () => {
    beforeEach(() => {
      show(profile({ bodyUrl: "https://example.com/body.png" }));
    });

    it("should show it in the avatar column", () => {
      expect(screen.getByAltText("Alice's avatar").getAttribute("src")).toBe("https://example.com/body.png");
    });
  });
});

describe("ProfileOverviewScreen badges", () => {
  describe("when the wallet has no profile", () => {
    beforeEach(() => {
      show(profile(), false, vi.fn(), false);
    });

    it("should not read badges", () => {
      expect(net.fetchUserBadges).not.toHaveBeenCalled();
    });

    it("should say it has none", () => {
      expect(screen.getByText("No badges yet. Jump in world and start collecting badges!")).toBeTruthy();
    });
  });

  describe("when the badges load", () => {
    beforeEach(async () => {
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
    });

    it("should show each badge with its tier, blurb and unlock date", async () => {
      expect(await screen.findByAltText("Explorer")).toBeTruthy();
      const tip = screen.getByRole("tooltip", { hidden: true });
      expect(tip.textContent).toContain("Gold");
      expect(tip.textContent).toContain("Visit 100 scenes");
      expect(tip.textContent).toContain("Unlocked on Mar 3, 2026");
    });

    it("should read the member's badges", async () => {
      await screen.findByAltText("Explorer");
      expect(net.fetchUserBadges).toHaveBeenCalledWith(A, { signal: expect.any(AbortSignal) });
    });
  });

  describe("when the badges loading ends with none", () => {
    let region: HTMLElement | undefined;

    beforeEach(() => {
      net.fetchUserBadges.mockResolvedValue([]);
      show(profile(), false, vi.fn(), true);
      region = screen.getAllByRole("status").find((el) => el.textContent === "Loading badges…");
    });

    it("should announce the loading in a live region", () => {
      expect(region).toBeTruthy();
    });

    it("should announce how the loading ended in the same live region", async () => {
      await waitFor(() => expect(region?.textContent).toBe("No badges yet. Jump in world and start collecting badges!"));
      expect(region?.isConnected).toBe(true);
      expect(region?.getAttribute("role")).toBe("status");
    });
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

  describe("when a background re-read of shown badges fails", () => {
    beforeEach(async () => {
      net.fetchUserBadges.mockResolvedValueOnce([{ id: "explorer", name: "Explorer", imageUrl: "https://example.com/e.png" }]);
      show(profile(), false, vi.fn(), true);
      await screen.findByAltText("Explorer");
      net.fetchUserBadges.mockRejectedValue(new Error("offline"));
      await act(async () => {
        await client.refetchQueries({ queryKey: ["profile", "badges"] });
      });
    });

    it("should record the error", () => {
      expect(client.getQueryCache().findAll({ queryKey: ["profile", "badges"] })[0]?.state.status).toBe("error");
    });

    it("should keep the badges on screen", async () => {
      await waitFor(() => expect(screen.getByAltText("Explorer")).toBeTruthy());
    });

    it("should not show the error state", async () => {
      await waitFor(() => expect(screen.getByAltText("Explorer")).toBeTruthy());
      expect(screen.queryByText("Badges are unavailable right now.")).toBeNull();
    });
  });

  describe("when the badges cannot be read", () => {
    beforeEach(() => {
      net.fetchUserBadges.mockRejectedValue(new Error("offline"));
      show(profile(), false, vi.fn(), true);
    });

    it("should say the badges are unavailable", async () => {
      await waitFor(() => expect(screen.getByText("Badges are unavailable right now.")).toBeTruthy());
    });

    it("should not say there are none", async () => {
      await waitFor(() => expect(screen.getByText("Badges are unavailable right now.")).toBeTruthy());
      expect(screen.queryByText("No badges yet. Jump in world and start collecting badges!")).toBeNull();
    });
  });
});
