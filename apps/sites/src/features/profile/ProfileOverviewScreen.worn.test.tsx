// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const net = vi.hoisted(() => ({ fetchWornCollectibles: vi.fn() }));
vi.mock("@/data/lib/catalyst/marketplace/profile-collectibles", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/marketplace/profile-collectibles")>()),
  fetchWornCollectibles: net.fetchWornCollectibles,
}));
vi.mock("@/data/lib/catalyst/social/badges", () => ({ fetchUserBadges: () => Promise.resolve([]) }));

import type { ProfileCreation } from "@/data/lib/catalyst/marketplace/profile-collectibles";
import { emptyProfile } from "@/data/lib/catalyst/social/profile";
import { profileKeys } from "@/data/lib/profile/queries";

import { ProfileOverviewScreen } from "./ProfileOverviewScreen";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const URNS = [
  "urn:decentraland:matic:collections-v2:0xabc:0",
  "urn:decentraland:matic:collections-v2:0xabc:1",
];
const tabs = [{ id: "overview", label: "Overview", href: `/profile/${A}` }];

function card(n: number, over: Partial<ProfileCreation> = {}): ProfileCreation {
  return {
    id: `0xabc-${n}`,
    contractAddress: "0xabc",
    itemId: `${n}`,
    name: `Jacket ${n}`,
    thumbnail: "https://example.com/item.png",
    rarity: "epic",
    network: "polygon",
    payment: { kind: "mana", mana: "10" },
    traits: { category: "upper_body", bodyShapes: ["BaseMale"], isSmart: false },
    ...over,
  };
}

function show({ worn = URNS, own = false }: { worn?: string[]; own?: boolean } = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const profile = { ...emptyProfile(A), name: "Alice", wornCollectibles: worn };
  const router = createMemoryRouter(
    [{ path: "*", element: <ProfileOverviewScreen profile={profile} hasProfile own={own} tabs={tabs} onTab={vi.fn()} /> }],
    { initialEntries: [`/profile/${A}`] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return { client, router };
}

/** The Equipped items section: its heading's parent. */
const section = () => screen.getByRole("heading", { level: 2, name: "Equipped items" }).parentElement as HTMLElement;

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("profile overview, equipped items", () => {
  it("shows a card for each collectible the avatar wears, read by its URNs", async () => {
    net.fetchWornCollectibles.mockResolvedValue([card(0), card(1, { payment: { kind: "none" } })]);

    const { client } = show();

    expect(within(section()).getByText("Loading equipped items")).toBeTruthy();
    expect(await within(section()).findByRole("link", { name: "Jacket 0" })).toBeTruthy();
    expect(within(section()).getAllByRole("listitem")).toHaveLength(2);
    expect(within(section()).getByRole("status").textContent).toBe("2 equipped items");
    expect(net.fetchWornCollectibles.mock.calls[0]?.[0]).toEqual(URNS);
    expect(client.getQueryData(profileKeys.wornCollectibles(URNS))).toHaveLength(2);
  });

  it("gives a visitor Buy for an item on sale and View in Shop for one that is not", async () => {
    net.fetchWornCollectibles.mockResolvedValue([card(0), card(1, { payment: { kind: "none" } })]);

    show();

    expect(await within(section()).findByRole("link", { name: "Buy Jacket 0" })).toBeTruthy();
    expect(within(section()).getByRole("link", { name: "View in Shop: Jacket 1" })).toBeTruthy();
    expect(within(section()).getByText("Not for sale")).toBeTruthy();
  });

  it("shows the member's own items without the buy actions", async () => {
    net.fetchWornCollectibles.mockResolvedValue([card(0)]);

    show({ own: true });

    expect(await within(section()).findByRole("link", { name: "Jacket 0" })).toBeTruthy();
    expect(within(section()).queryByRole("link", { name: "Buy Jacket 0" })).toBeNull();
  });

  it("says there are no equipped items for an avatar that wears no collectible, without a request", () => {
    show({ worn: [] });

    expect(within(section()).getByText("No equipped items yet.")).toBeTruthy();
    expect(net.fetchWornCollectibles).not.toHaveBeenCalled();
  });

  it("says there are no equipped items when the catalogue knows none of them", async () => {
    net.fetchWornCollectibles.mockResolvedValue([]);

    show();

    expect(await within(section()).findByText("No equipped items yet.")).toBeTruthy();
  });

  it("says when the items cannot be read, never an empty list, and reads again on Try again", async () => {
    const user = userEvent.setup();
    net.fetchWornCollectibles.mockRejectedValueOnce(new Error("502"));

    show();

    expect(await within(section()).findByText("Equipped items are unavailable right now.")).toBeTruthy();
    expect(within(section()).queryByText("No equipped items yet.")).toBeNull();
    net.fetchWornCollectibles.mockResolvedValue([card(0)]);
    await user.click(within(section()).getByRole("button", { name: "Try again" }));

    expect(await within(section()).findByRole("link", { name: "Jacket 0" })).toBeTruthy();
  });
});
