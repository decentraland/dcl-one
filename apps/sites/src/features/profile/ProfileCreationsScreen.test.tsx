// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
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
const net = vi.hoisted(() => ({ fetchProfileCreations: vi.fn() }));
vi.mock("@/data/lib/catalyst/marketplace/profile-collectibles", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/marketplace/profile-collectibles")>()),
  fetchProfileCreations: net.fetchProfileCreations,
}));
vi.mock("@/data/lib/auth/index", () => ({ useAuth: () => ({ identity: null, isConnected: false, address: null }) }));

import type { ProfileCreation } from "@/data/lib/catalyst/marketplace/profile-collectibles";
import { emptyProfile, type ProfileVM } from "@/data/lib/catalyst/social/profile";
import { PROFILE_COLLECTIBLES_PAGE, profileKeys } from "@/data/lib/profile/queries";

import { ProfileCreationsScreen } from "./ProfileCreationsScreen";

const A = "0x09bc69a986e1b37fb01ef36124f4c2e14fa0ab6b";
const tabs = [
  { id: "overview", label: "Overview", href: `/profile/${A}` },
  { id: "creations", label: "Creations", href: `/profile/${A}/creations` },
];

function creation(n: number, over: Partial<ProfileCreation> = {}): ProfileCreation {
  return {
    id: `0xabc-${n}`,
    contractAddress: "0xabc",
    itemId: `${n}`,
    name: `Hat ${n}`,
    thumbnail: "https://example.com/hat.png",
    rarity: "legendary",
    network: "polygon",
    price: "369",
    traits: { category: "hat", bodyShapes: ["BaseMale", "BaseFemale"], isSmart: true },
    ...over,
  };
}

const page = (items: ProfileCreation[], total = items.length) => ({ items, received: items.length, total });

function profile(over: Partial<ProfileVM> = {}): ProfileVM {
  return { ...emptyProfile(A), name: "Billyjuice", nameColor: "#ff8362", ...over };
}

function show(vm: ProfileVM = profile(), { own = false } = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(
    [{ path: "*", element: <ProfileCreationsScreen profile={vm} own={own} tabs={tabs} onTab={vi.fn()} /> }],
    { initialEntries: [`/profile/${A}/creations`] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return client;
}

const card = (name: string) => screen.getByRole("link", { name }).closest("article") as HTMLElement;

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("profile creations", () => {
  it("opens on the member's wearables, each card linking to its Shop item", async () => {
    net.fetchProfileCreations.mockResolvedValue(page([creation(0)]));

    show();

    const link = await screen.findByRole("link", { name: "Hat 0" });
    expect(link.getAttribute("href")).toBe("/shop/item/0xabc/0");
    expect(net.fetchProfileCreations.mock.calls[0]?.[0]).toBe(A);
    expect(net.fetchProfileCreations.mock.calls[0]?.[1]).toMatchObject({ category: "wearable", skip: 0, first: PROFILE_COLLECTIBLES_PAGE });
    const hat = card("Hat 0");
    expect(within(hat).getByText("By Billyjuice")).toBeTruthy();
    expect(within(hat).getByText("Legendary")).toBeTruthy();
    expect(within(hat).getByText((_, node) => node?.tagName === "P" && node.textContent === "Price: 369 MANA")).toBeTruthy();
    expect(within(hat).getByRole("img", { name: "Hat" })).toBeTruthy();
    expect(within(hat).getByRole("img", { name: "Unisex" })).toBeTruthy();
    expect(within(hat).getByRole("img", { name: "Smart wearable" })).toBeTruthy();
    expect(within(hat).getByRole("link", { name: "Buy Hat 0" }).getAttribute("href")).toBe("/shop/item/0xabc/0");
    expect(screen.getByRole("link", { name: "View all" }).getAttribute("href")).toBe(`/shop/accounts/${A}`);
    // Announced in the live region when loading ends.
    expect(screen.getByText("1 creation").closest('[role="status"]')).not.toBeNull();
  });

  it("says an item is not for sale and offers the Shop page instead of Buy", async () => {
    net.fetchProfileCreations.mockResolvedValue(page([creation(0, { price: undefined })]));

    show();

    const hat = await screen.findByRole("link", { name: "Hat 0" }).then(() => card("Hat 0"));
    expect(within(hat).getByText("Not for sale")).toBeTruthy();
    expect(within(hat).queryByRole("link", { name: "Buy Hat 0" })).toBeNull();
    expect(within(hat).getByRole("link", { name: "View Hat 0 in Shop" }).getAttribute("href")).toBe("/shop/item/0xabc/0");
  });

  it("shows a card whose image, rarity and traits are missing without inventing them", async () => {
    net.fetchProfileCreations.mockResolvedValue(
      page([creation(0, { thumbnail: undefined, rarity: undefined, traits: { bodyShapes: [], isSmart: false } })]),
    );

    show();

    const hat = await screen.findByRole("link", { name: "Hat 0" }).then(() => card("Hat 0"));
    expect(hat.querySelector("img")).toBeNull();
    expect(within(hat).queryByText("Common")).toBeNull();
    expect(within(hat).queryAllByRole("img")).toHaveLength(0);
  });

  it("names the creator by short address while the profile has no name", async () => {
    net.fetchProfileCreations.mockResolvedValue(page([creation(0)]));

    show(emptyProfile(A));

    expect(await screen.findByText("By 0x09bc\u2026ab6b")).toBeTruthy();
  });

  it("switches to emotes, reading that category", async () => {
    const user = userEvent.setup();
    net.fetchProfileCreations.mockImplementation(async (_a: string, params: { category: string }) =>
      page(params.category === "emote" ? [creation(9, { name: "Dance" })] : [creation(0)]),
    );
    show();
    await screen.findByRole("link", { name: "Hat 0" });

    await user.click(screen.getByRole("button", { name: "Emotes" }));

    expect(await screen.findByRole("link", { name: "Dance" })).toBeTruthy();
    expect(screen.queryByRole("link", { name: "Hat 0" })).toBeNull();
    expect(screen.getByRole("button", { name: "Emotes" }).getAttribute("aria-pressed")).toBe("true");
  });

  it("tells a visitor the member has created nothing", async () => {
    net.fetchProfileCreations.mockResolvedValue(page([]));

    show();

    expect(await screen.findByText("This account has not created anything yet.")).toBeTruthy();
  });

  it("tells the owner their creations will appear here", async () => {
    net.fetchProfileCreations.mockResolvedValue(page([]));

    show(profile(), { own: true });

    expect(await screen.findByText("Items you create will appear here.")).toBeTruthy();
  });

  it("says the read failed, never an empty list, and tries again", async () => {
    const user = userEvent.setup();
    net.fetchProfileCreations.mockRejectedValueOnce(new Error("503")).mockResolvedValueOnce(page([creation(0)]));
    show();

    expect(await screen.findByText("Creations are unavailable right now.")).toBeTruthy();
    expect(screen.queryByText("This account has not created anything yet.")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Try again" }));

    expect(await screen.findByRole("link", { name: "Hat 0" })).toBeTruthy();
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("link", { name: "Hat 0" })));
  });

  it("keeps the list on screen when a background re-read fails", async () => {
    net.fetchProfileCreations.mockResolvedValueOnce(page([creation(0)])).mockRejectedValueOnce(new Error("503"));
    const client = show();
    await screen.findByRole("link", { name: "Hat 0" });

    await act(async () => {
      await client.refetchQueries({ queryKey: profileKeys.creations(A, "wearable") });
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(client.getQueryState(profileKeys.creations(A, "wearable"))?.status).toBe("error");
    expect(screen.getByRole("link", { name: "Hat 0" })).toBeTruthy();
    expect(screen.queryByText("Creations are unavailable right now.")).toBeNull();
  });

  it("loads more after the rows shown, once each, and stops at the catalogue's total", async () => {
    const user = userEvent.setup();
    const first = Array.from({ length: PROFILE_COLLECTIBLES_PAGE }, (_, n) => creation(n));
    // A newer item shifted the offsets: the second page repeats the first page's last row.
    net.fetchProfileCreations
      .mockResolvedValueOnce(page(first, 26))
      .mockResolvedValueOnce(page([creation(PROFILE_COLLECTIBLES_PAGE - 1), creation(24), creation(25)], 26));
    show();
    await screen.findByRole("link", { name: "Hat 0" });

    const button = screen.getByRole("button", { name: "Load more" });
    await user.click(button);

    expect(await screen.findByRole("link", { name: "Hat 25" })).toBeTruthy();
    expect(net.fetchProfileCreations.mock.calls[1]?.[1]).toMatchObject({ skip: PROFILE_COLLECTIBLES_PAGE });
    // The list ended, so the button went away: focus moves to the first new card, not the page body.
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("link", { name: "Hat 24" })));
    expect(screen.getAllByRole("link", { name: `Hat ${PROFILE_COLLECTIBLES_PAGE - 1}` })).toHaveLength(1);
    expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
  });

  it("keeps focus on Load more while the next page loads, without asking twice", async () => {
    const user = userEvent.setup();
    const first = Array.from({ length: PROFILE_COLLECTIBLES_PAGE }, (_, n) => creation(n));
    let release!: (value: unknown) => void;
    net.fetchProfileCreations
      .mockResolvedValueOnce(page(first, 60))
      .mockReturnValueOnce(new Promise((resolve) => (release = resolve)));
    show();
    await screen.findByRole("link", { name: "Hat 0" });
    const button = screen.getByRole("button", { name: "Load more" });

    await user.click(button);

    expect(await screen.findByText("Loading more creations")).toBeTruthy();
    expect(document.activeElement).toBe(button);
    expect(button.getAttribute("aria-disabled")).toBe("true");
    await user.click(button);
    expect(net.fetchProfileCreations).toHaveBeenCalledTimes(2);
    await act(async () => release(page([creation(24)], 60)));
    expect(await screen.findByRole("link", { name: "Hat 24" })).toBeTruthy();
    // More remain, so the button stays and keeps the focus.
    expect(document.activeElement).toBe(button);
  });

  it("keeps the loaded rows when the next page fails, and retries it", async () => {
    const user = userEvent.setup();
    const first = Array.from({ length: PROFILE_COLLECTIBLES_PAGE }, (_, n) => creation(n));
    net.fetchProfileCreations
      .mockResolvedValueOnce(page(first, 30))
      .mockRejectedValueOnce(new Error("503"))
      .mockResolvedValueOnce(page([creation(24)], 30));
    show();
    await screen.findByRole("link", { name: "Hat 0" });

    await user.click(screen.getByRole("button", { name: "Load more" }));

    // Announced, and the same button keeps the focus while it turns into Try again.
    const failure = await screen.findByText("More creations could not be loaded.");
    expect(failure.getAttribute("role")).toBe("status");
    expect(screen.getByRole("link", { name: "Hat 0" })).toBeTruthy();
    const retry = screen.getByRole("button", { name: "Try again" });
    expect(document.activeElement).toBe(retry);
    await user.click(retry);
    await waitFor(() => expect(screen.getByRole("link", { name: "Hat 24" })).toBeTruthy());
  });
});
