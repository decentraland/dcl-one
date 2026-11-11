// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
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
const net = vi.hoisted(() => ({ fetchProfileAssets: vi.fn() }));
vi.mock("@/data/lib/catalyst/marketplace/profile-collectibles", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/marketplace/profile-collectibles")>()),
  fetchProfileAssets: net.fetchProfileAssets,
}));
vi.mock("@/data/lib/auth/index", () => ({ useAuth: () => ({ identity: null, isConnected: false, address: null }) }));

import { CatalogCard } from "@dcl/one-ui/apps/sites/provisional/CatalogCard/CatalogCard";

import type { ProfileAsset } from "@/data/lib/catalyst/marketplace/profile-collectibles";
import { emptyProfile, type ProfileVM } from "@/data/lib/catalyst/social/profile";
import { PROFILE_COLLECTIBLES_PAGE, profileKeys } from "@/data/lib/profile/queries";

import { ProfileAssetsScreen } from "./ProfileAssetsScreen";

const A = "0x09bc69a986e1b37fb01ef36124f4c2e14fa0ab6b";
const tabs = [
  { id: "overview", label: "Overview", href: `/profile/${A}` },
  { id: "assets", label: "My Assets", href: `/profile/${A}/assets` },
];

function asset(n: number, over: Partial<ProfileAsset> = {}): ProfileAsset {
  return {
    id: `0xabc-${n}`,
    contractAddress: "0xabc",
    tokenId: `${n}`,
    category: "wearable",
    name: `Power Up ${n}`,
    image: "https://example.com/token.png",
    rarity: "epic",
    network: "polygon",
    traits: { category: "top_head", bodyShapes: ["BaseFemale"], isSmart: false },
    ...over,
  };
}

const name = (n: number, label: string) =>
  asset(n, { contractAddress: "0xens", category: "ens", name: label, image: undefined, rarity: undefined, traits: { bodyShapes: [], isSmart: false } });

const page = (items: ProfileAsset[], total = items.length) => ({ items, received: items.length, total });

function profile(): ProfileVM {
  return { ...emptyProfile(A), name: "Billyjuice", nameColor: "#ff8362" };
}

function show({ own = true } = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(
    [{ path: "*", element: <ProfileAssetsScreen profile={profile()} own={own} tabs={tabs} onTab={vi.fn()} /> }],
    { initialEntries: [`/profile/${A}/assets`] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return client;
}

const card = (label: string) => screen.getByRole("link", { name: label }).closest("article") as HTMLElement;

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("profile assets", () => {
  it("opens on every category, with the total and each token linking to its Shop page", async () => {
    net.fetchProfileAssets.mockResolvedValue(page([asset(1, { price: "12" })], 3431));

    show();

    const link = await screen.findByRole("link", { name: "Power Up 1" });
    expect(link.getAttribute("href")).toBe("/shop/token/0xabc/1");
    expect(net.fetchProfileAssets.mock.calls[0]?.[1]).toMatchObject({ category: undefined, skip: 0, first: PROFILE_COLLECTIBLES_PAGE });
    expect(screen.getByText("3431 assets")).toBeTruthy();
    expect(screen.getByRole("button", { name: "All" }).getAttribute("aria-pressed")).toBe("true");
    const token = card("Power Up 1");
    expect(within(token).getByText("Epic")).toBeTruthy();
    expect(within(token).getByRole("img", { name: "Top Head" })).toBeTruthy();
    expect(within(token).getByRole("img", { name: "For female" })).toBeTruthy();
    expect(within(token).getByRole("link", { name: "View Power Up 1" }).getAttribute("href")).toBe("/shop/token/0xabc/1");
  });

  it("gives a NAME in the full list its Edit and Transfer instead of View", async () => {
    net.fetchProfileAssets.mockResolvedValue(page([name(5, "brai")]));

    show();

    const ens = await screen.findByRole("link", { name: "brai" }).then(() => card("brai"));
    expect(within(ens).queryByText("Common")).toBeNull();
    expect(within(ens).getByRole("link", { name: "Edit brai" }).getAttribute("href")).toBe("/create/names");
    expect(within(ens).getByRole("link", { name: "Transfer brai" }).getAttribute("href")).toBe("/shop/token/0xens/5/transfer");
    expect(within(ens).queryByRole("link", { name: /^View/ })).toBeNull();
    expect(screen.getByText("1 asset")).toBeTruthy();
  });

  it("shows a NAME's glyph where the catalogue has no image", async () => {
    net.fetchProfileAssets.mockResolvedValue(page([name(5, "brai")]));

    show();

    const ens = await screen.findByRole("link", { name: "brai" }).then(() => card("brai"));
    expect(ens.querySelector("img")).toBeNull();
    expect(ens.querySelector("svg[aria-hidden], [aria-hidden] svg")).not.toBeNull();
  });

  it("drops a card image that fails to load, showing its glyph instead", () => {
    const { container } = render(
      <CatalogCard title="Broken" href="/shop/token/0xabc/1" imageUrl="https://example.com/missing.png" placeholder={<svg data-glyph="" />} />,
    );

    fireEvent.error(container.querySelector("img") as HTMLImageElement);

    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("[data-glyph]")).not.toBeNull();
  });

  it("lists NAMEs as rows under the Names filter", async () => {
    const user = userEvent.setup();
    net.fetchProfileAssets.mockImplementation(async (_a: string, params: { category?: string }) =>
      page(params.category === "ens" ? [name(5, "brai")] : [asset(1)]),
    );
    show();
    await screen.findByRole("link", { name: "Power Up 1" });

    await user.click(screen.getByRole("button", { name: "Names" }));

    expect(await screen.findByText(".dcl.eth")).toBeTruthy();
    expect(screen.getByText("brai")).toBeTruthy();
    expect(screen.queryByRole("article")).toBeNull();
    expect(screen.getByRole("link", { name: "Transfer brai" }).getAttribute("href")).toBe("/shop/token/0xens/5/transfer");
    expect(net.fetchProfileAssets.mock.calls.at(-1)?.[1]).toMatchObject({ category: "ens" });
  });

  it("says a filtered category holds nothing", async () => {
    const user = userEvent.setup();
    net.fetchProfileAssets.mockImplementation(async (_a: string, params: { category?: string }) =>
      page(params.category ? [] : [asset(1)]),
    );
    show();
    await screen.findByRole("link", { name: "Power Up 1" });

    await user.click(screen.getByRole("button", { name: "Estates" }));

    expect(await screen.findByText("0 assets")).toBeTruthy();
    expect(screen.queryByText("No assets yet")).toBeNull();
  });

  it("sends an owner with nothing to the Shop", async () => {
    net.fetchProfileAssets.mockResolvedValue(page([]));

    show();

    expect(await screen.findByText("No assets yet", { selector: "p" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Go to the Shop" }).getAttribute("href")).toBe("/shop");
  });

  it("tells a visitor what would appear, without the owner's call to action", async () => {
    net.fetchProfileAssets.mockResolvedValue(page([]));

    show({ own: false });

    expect(await screen.findByText("Wearables, names and lands you own will appear here.")).toBeTruthy();
    expect(screen.queryByRole("link", { name: "Go to the Shop" })).toBeNull();
  });

  it("says the read failed, never an empty list, and tries again", async () => {
    const user = userEvent.setup();
    net.fetchProfileAssets.mockRejectedValueOnce(new Error("503")).mockResolvedValueOnce(page([asset(1)]));
    show();

    expect(await screen.findByText("Assets are unavailable right now.")).toBeTruthy();
    expect(screen.queryByText("No assets yet")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Try again" }));

    expect(await screen.findByRole("link", { name: "Power Up 1" })).toBeTruthy();
  });

  it("keeps the list on screen when a background re-read fails", async () => {
    net.fetchProfileAssets.mockResolvedValueOnce(page([asset(1)])).mockRejectedValueOnce(new Error("503"));
    const client = show();
    await screen.findByRole("link", { name: "Power Up 1" });

    await act(async () => {
      await client.refetchQueries({ queryKey: profileKeys.assets(A, null) });
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(client.getQueryState(profileKeys.assets(A, null))?.status).toBe("error");
    expect(screen.getByRole("link", { name: "Power Up 1" })).toBeTruthy();
    expect(screen.queryByText("Assets are unavailable right now.")).toBeNull();
  });

  it("loads the next page under the first", async () => {
    const user = userEvent.setup();
    const first = Array.from({ length: PROFILE_COLLECTIBLES_PAGE }, (_, n) => asset(n));
    net.fetchProfileAssets.mockResolvedValueOnce(page(first, 25)).mockResolvedValueOnce(page([asset(24)], 25));
    show();
    await screen.findByRole("link", { name: "Power Up 0" });

    await user.click(screen.getByRole("button", { name: "Load more" }));

    expect(await screen.findByRole("link", { name: "Power Up 24" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Power Up 0" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
  });
});
