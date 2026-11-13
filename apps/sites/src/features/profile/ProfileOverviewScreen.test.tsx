// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

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

import { GET_A_NAME_HREF, MANAGE_WORLD_HREF, ProfileOverviewScreen } from "./ProfileOverviewScreen";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const tabs = [
  { id: "overview", label: "Overview", href: `/profile/${A}` },
  { id: "places", label: "Places", href: `/profile/${A}/places` },
];

function profile(over: Partial<ProfileVM> = {}): ProfileVM {
  return { ...emptyProfile(A), name: "Alice", nameColor: "#ff8362", ...over };
}

function show(vm: ProfileVM, own = false, onTab = vi.fn(), hasProfile = false) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
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

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("profile overview screen, header", () => {
  it("shows a claimed name with the verified mark and no number suffix", () => {
    show(profile({ hasClaimedName: true }));

    expect(screen.getByRole("heading", { level: 1, name: "Alice" })).toBeTruthy();
    expect(screen.getByLabelText("Verified")).toBeTruthy();
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
    Object.assign(navigator, { clipboard: { writeText } });
    show(profile());

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Copy address" }));
    });

    expect(writeText).toHaveBeenCalledWith(A);
    expect(screen.getByRole("status").textContent).toContain("Address copied!");
  });

  it("offers the owner a NAME, or the worlds once they have one, and a visitor neither", () => {
    show(profile({ hasClaimedName: false }), true);
    expect(screen.getByRole("link", { name: "Get a Unique Name" }).getAttribute("href")).toBe(GET_A_NAME_HREF);
    cleanup();

    show(profile({ hasClaimedName: true }), true);
    expect(screen.getByRole("link", { name: "Manage World" }).getAttribute("href")).toBe(MANAGE_WORLD_HREF);
    cleanup();

    show(profile(), false);
    expect(screen.queryByRole("link", { name: /Unique Name|Manage World/ })).toBeNull();
  });
});

describe("profile overview screen, tabs", () => {
  it("marks Overview as the current page and navigates in the app on a tab", async () => {
    const onTab = vi.fn();
    const router = show(profile(), false, onTab);
    const nav = screen.getByRole("navigation", { name: "Profile sections" });

    expect(within(nav).getByRole("link", { name: "Overview" }).getAttribute("aria-current")).toBe("page");
    await act(async () => {
      fireEvent.click(within(nav).getByRole("link", { name: "Places" }));
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

  it("links only http(s) URLs, named by their title or their site", () => {
    show(
      profile({
        links: [
          { title: "My blog", url: "https://blog.example.com/" },
          { title: "https://www.example.org/me", url: "https://www.example.org/me" },
          { title: "click", url: "javascript:alert(1)" },
        ],
      }),
    );

    const links = within(screen.getByRole("heading", { name: "Links" }).parentElement as HTMLElement).getAllByRole("link");
    expect(links.map((link) => [link.textContent, link.getAttribute("href"), link.getAttribute("rel")])).toEqual([
      ["My blog", "https://blog.example.com/", "noopener noreferrer"],
      ["Example", "https://www.example.org/me", "noopener noreferrer"],
    ]);
  });

  it("leaves the Links section out when no link is safe", () => {
    show(profile({ links: [{ title: "x", url: "data:text/html,hi" }] }));

    expect(screen.queryByRole("heading", { name: "Links" })).toBeNull();
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

  it("falls back to the empty text when the badges cannot be read", async () => {
    net.fetchUserBadges.mockRejectedValue(new Error("offline"));

    show(profile(), false, vi.fn(), true);

    await waitFor(() =>
      expect(screen.getByText("No badges yet. Jump in world and start collecting badges!")).toBeTruthy(),
    );
  });
});

describe("provisional Sites UI inventory", () => {
  it("lists every provisional component, and each carries its TODO(figma-ds) mark", () => {
    const root = join(__dirname, "../../../../../library/ui/apps/sites/provisional");
    const readme = readFileSync(join(root, "README.md"), "utf8");
    const files: string[] = [];
    const walk = (dir: string) => {
      for (const entry of readdirSync(dir)) {
        const path = join(dir, entry);
        if (statSync(path).isDirectory()) walk(path);
        else if (/\.(tsx|module\.css)$/.test(entry)) files.push(path);
      }
    };
    walk(root);

    expect(files.length).toBeGreaterThan(0);
    for (const file of files) {
      expect(readFileSync(file, "utf8"), file).toContain("TODO(figma-ds)");
      if (file.endsWith(".tsx")) {
        const name = relative(root, file).replace(/\.tsx$/, "");
        expect(readme, `${name} is missing from the inventory`).toContain(`\`${name}\``);
      }
    }
  });
});
