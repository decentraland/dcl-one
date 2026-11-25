// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const net = vi.hoisted(() => ({ fetchProfileAssets: vi.fn(), fetchProfileAssetCount: vi.fn() }));
vi.mock("@/data/lib/catalyst/marketplace/profile-collectibles", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/marketplace/profile-collectibles")>()),
  fetchProfileAssets: net.fetchProfileAssets,
  fetchProfileAssetCount: net.fetchProfileAssetCount,
}));
vi.mock("@/data/lib/auth/index", () => ({ useAuth: () => ({ identity: null, isConnected: false, address: null }) }));

import { ShopCard } from "@dcl/one-ui";

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
  return { client, router };
}

const card = (label: string) => screen.getByRole("link", { name: label }).closest("article") as HTMLElement;

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("ProfileAssetsScreen", () => {
  let user: ReturnType<typeof userEvent.setup>;

  beforeEach(() => {
    user = userEvent.setup();
    net.fetchProfileAssetCount.mockResolvedValue(1);
  });

  describe("when the assets load", () => {
    let link: HTMLElement;

    beforeEach(async () => {
      net.fetchProfileAssets.mockResolvedValue(page([asset(1, { price: "12" })], 3431));
      show();
      link = await screen.findByRole("link", { name: "Power Up 1" });
    });

    it("should link each token to its Shop page", () => {
      expect(link.getAttribute("href")).toBe("/shop/token/0xabc/1");
    });

    it("should open on every category, from the first page", () => {
      expect(net.fetchProfileAssets.mock.calls[0]?.[1]).toMatchObject({ category: undefined, skip: 0, first: PROFILE_COLLECTIBLES_PAGE });
    });

    it("should show the total", () => {
      expect(screen.getByText("3431 assets")).toBeTruthy();
    });

    it("should select the All filter", () => {
      expect(screen.getByRole("tab", { name: "All" }).getAttribute("aria-selected")).toBe("true");
    });

    it("should show the token's rarity, category and body shape", () => {
      const token = card("Power Up 1");
      expect(within(token).getByText("epic")).toBeTruthy();
      expect(within(token).getByRole("img", { name: "Top Head" })).toBeTruthy();
      expect(within(token).getByRole("img", { name: "For female" })).toBeTruthy();
    });

    it("should link the token's View action to its Shop page", () => {
      expect(within(card("Power Up 1")).getByRole("link", { name: "View Power Up 1" }).getAttribute("href")).toBe("/shop/token/0xabc/1");
    });
  });

  describe("when the full list holds a NAME", () => {
    let ens: HTMLElement;

    beforeEach(async () => {
      net.fetchProfileAssets.mockResolvedValue(page([name(5, "brai")]));
      show();
      ens = await screen.findByRole("link", { name: "brai" }).then(() => card("brai"));
    });

    it("should show no rarity", () => {
      expect(within(ens).queryByText("common")).toBeNull();
    });

    it("should give it its View action", () => {
      expect(within(ens).getByRole("link", { name: "View brai" }).getAttribute("href")).toBe("/shop/token/0xens/5");
    });

    it("should give it its Transfer action", () => {
      expect(within(ens).getByRole("link", { name: "Transfer brai" }).getAttribute("href")).toBe("/shop/token/0xens/5/transfer");
    });

    it("should count one asset", () => {
      expect(screen.getByText("1 asset")).toBeTruthy();
    });

    describe("and the catalogue has no image for it", () => {
      it("should show the NAME's glyph instead of an image", () => {
        expect(ens.querySelector("img")).toBeNull();
        expect(ens.querySelector("svg[aria-hidden], [aria-hidden] svg")).not.toBeNull();
      });
    });
  });

  describe("when a card image fails to load", () => {
    let container: HTMLElement;

    beforeEach(() => {
      ({ container } = render(
        <ShopCard name="Broken" href="/shop/token/0xabc/1" image="https://example.com/missing.png" placeholder={<svg data-glyph="" />} />,
      ));
      fireEvent.error(container.querySelector("img") as HTMLImageElement);
    });

    it("should drop the image", () => {
      expect(container.querySelector("img")).toBeNull();
    });

    it("should show its glyph instead", () => {
      expect(container.querySelector("[data-glyph]")).not.toBeNull();
    });
  });

  describe("when the owner has Lands and Estates", () => {
    let land: ProfileAsset;
    let estate: ProfileAsset;

    beforeEach(async () => {
      land = asset(3, { category: "parcel", name: "Parcel 162,-133", rarity: undefined, traits: { bodyShapes: [], isSmart: false } });
      estate = asset(4, { category: "estate", name: "Estate", rarity: undefined, traits: { bodyShapes: [], isSmart: false } });
      net.fetchProfileAssets.mockImplementation(async (_a: string, params: { category?: string }) =>
        page(params.category === "estate" ? [estate] : params.category === "parcel" ? [land] : [asset(1)]),
      );
      show();
      await screen.findByRole("link", { name: "Power Up 1" });
    });

    describe("and the Lands filter is chosen", () => {
      let parcel: HTMLElement;

      beforeEach(async () => {
        await user.click(screen.getByRole("tab", { name: "Lands" }));
        parcel = await screen.findByRole("link", { name: "Parcel 162,-133" }).then(() => card("Parcel 162,-133"));
      });

      it("should show the Land as a named card with its View action", () => {
        expect(within(parcel).getByRole("link", { name: "View Parcel 162,-133" }).getAttribute("href")).toBe("/shop/token/0xabc/3");
      });

      it("should show no rarity", () => {
        expect(within(parcel).queryByText("common")).toBeNull();
      });

      describe("and then the Estates filter is chosen", () => {
        let plot: HTMLElement;

        beforeEach(async () => {
          await user.click(screen.getByRole("tab", { name: "Estates" }));
          plot = await screen.findByRole("link", { name: "Estate" }).then(() => card("Estate"));
        });

        it("should show the Estate as a named card with its View action", () => {
          expect(within(plot).getByRole("link", { name: "View Estate" }).getAttribute("href")).toBe("/shop/token/0xabc/4");
        });
      });
    });
  });

  describe("when the Names filter is chosen", () => {
    beforeEach(async () => {
      net.fetchProfileAssets.mockImplementation(async (_a: string, params: { category?: string }) =>
        page(params.category === "ens" ? [name(5, "brai")] : [asset(1)]),
      );
      show();
      await screen.findByRole("link", { name: "Power Up 1" });
      await user.click(screen.getByRole("tab", { name: "Names" }));
    });

    it("should list the NAMEs as rows, not cards", async () => {
      expect(await screen.findByText(".dcl.eth")).toBeTruthy();
      expect(screen.getByText("brai")).toBeTruthy();
      expect(screen.queryByRole("article")).toBeNull();
    });

    it("should give a NAME row its Transfer action", async () => {
      await screen.findByText(".dcl.eth");
      expect(screen.getByRole("link", { name: "Transfer brai" }).getAttribute("href")).toBe("/shop/token/0xens/5/transfer");
    });

    it("should read the Names category", async () => {
      await screen.findByText(".dcl.eth");
      expect(net.fetchProfileAssets.mock.calls.at(-1)?.[1]).toMatchObject({ category: "ens" });
    });
  });

  describe("when the catalogue reports category totals", () => {
    beforeEach(() => {
      net.fetchProfileAssets.mockResolvedValue(page([asset(1)], 30));
      net.fetchProfileAssetCount.mockImplementation(async (_owner: string, category: string) =>
        category === "ens" ? 1 : 0,
      );
      show();
    });

    it("should show a category with inventory beyond the loaded page", async () => {
      expect(await screen.findByRole("tab", { name: "Names" })).toBeTruthy();
    });

    it("should hide categories with no inventory", async () => {
      await screen.findByRole("tab", { name: "Names" });
      expect(screen.queryByRole("tab", { name: "Lands" })).toBeNull();
      expect(screen.queryByRole("tab", { name: "Estates" })).toBeNull();
    });
  });

  describe("when the catalogue cannot confirm a category total", () => {
    beforeEach(() => {
      net.fetchProfileAssets.mockResolvedValue(page([asset(1)]));
      net.fetchProfileAssetCount.mockImplementation(async (_owner: string, category: string) => {
        if (category === "parcel") throw new Error("Unavailable");
        return 0;
      });
      show();
    });

    it("should leave the unverified category accessible", async () => {
      expect(await screen.findByRole("tab", { name: "Lands" })).toBeTruthy();
      expect(screen.queryByRole("tab", { name: "Estates" })).toBeNull();
    });
  });

  describe("when a filtered category holds nothing", () => {
    beforeEach(async () => {
      net.fetchProfileAssets.mockImplementation(async (_a: string, params: { category?: string }) =>
        page(params.category ? [] : [asset(1)]),
      );
      show();
      await screen.findByRole("link", { name: "Power Up 1" });
      await user.click(screen.getByRole("tab", { name: "Estates" }));
    });

    it("should count zero assets", async () => {
      expect(await screen.findByText("0 assets")).toBeTruthy();
    });

    it("should not show the empty owner state", async () => {
      await screen.findByText("0 assets");
      expect(screen.queryByText("No assets yet")).toBeNull();
    });
  });

  describe("when every row of a page fails to read", () => {
    beforeEach(() => {
      net.fetchProfileAssets.mockResolvedValue({ items: [], received: 3, total: 3 });
      show();
    });

    it("should say the page could not be read", async () => {
      expect(await screen.findByText("These assets could not be read right now.")).toBeTruthy();
    });

    it("should not say there are none", async () => {
      await screen.findByText("These assets could not be read right now.");
      expect(screen.queryByText("No assets yet")).toBeNull();
    });
  });

  describe("when a single unreadable page has no next page to load", () => {
    beforeEach(async () => {
      net.fetchProfileAssets.mockResolvedValueOnce({ items: [], received: 3, total: 3 }).mockResolvedValueOnce(page([asset(1)]));
      show();
      await screen.findByText("These assets could not be read right now.");
    });

    it("should not offer Load more", () => {
      expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
    });

    describe("and Try again is clicked", () => {
      beforeEach(async () => {
        await user.click(screen.getByRole("button", { name: "Try again" }));
      });

      it("should read the page again and show it", async () => {
        expect(await screen.findByRole("link", { name: "Power Up 1" })).toBeTruthy();
        expect(net.fetchProfileAssets).toHaveBeenCalledTimes(2);
      });
    });
  });

  describe("when trying again on a single unreadable page", () => {
    let fail: (reason: Error) => void;

    beforeEach(async () => {
      fail = () => {};
      net.fetchProfileAssets
        .mockResolvedValueOnce({ items: [], received: 3, total: 3 })
        .mockReturnValueOnce(new Promise((_, reject) => (fail = reject)));
      show();
      await screen.findByText("These assets could not be read right now.");
      await user.click(screen.getByRole("button", { name: "Try again" }));
    });

    describe("and the re-read is pending", () => {
      it("should say it is reading again", async () => {
        expect(await screen.findByText("Reading the assets again.")).toBeTruthy();
      });

      it("should mark Try again busy", async () => {
        await screen.findByText("Reading the assets again.");
        expect(screen.getByRole("button", { name: "Try again" }).getAttribute("aria-busy")).toBe("true");
      });
    });

    describe("and the re-read fails", () => {
      beforeEach(async () => {
        await screen.findByText("Reading the assets again.");
        await act(async () => fail(new Error("503")));
      });

      it("should say trying again failed", async () => {
        expect(await screen.findByText("These assets could not be read right now. Trying again failed.")).toBeTruthy();
      });

      it("should no longer mark Try again busy", async () => {
        await screen.findByText("These assets could not be read right now. Trying again failed.");
        expect(screen.getByRole("button", { name: "Try again" }).getAttribute("aria-busy")).not.toBe("true");
      });
    });
  });

  describe("when a re-read the reader did not ask for runs on an unreadable page", () => {
    let fail: (reason: Error) => void;

    beforeEach(async () => {
      fail = () => {};
      net.fetchProfileAssets
        .mockResolvedValueOnce({ items: [], received: 3, total: 3 })
        .mockReturnValueOnce(new Promise((_, reject) => (fail = reject)));
      const { client } = show();
      await screen.findByText("These assets could not be read right now.");
      // A background re-read, e.g. on window focus.
      act(() => void client.refetchQueries());
      await waitFor(() => expect(net.fetchProfileAssets).toHaveBeenCalledTimes(2));
    });

    describe("and it is pending", () => {
      it("should not say it is reading again", () => {
        expect(screen.queryByText("Reading the assets again.")).toBeNull();
      });

      it("should keep the first result's copy", () => {
        expect(screen.getByText("These assets could not be read right now.")).toBeTruthy();
      });
    });

    describe("and it fails", () => {
      beforeEach(async () => {
        await act(async () => fail(new Error("503")));
        await act(async () => new Promise((resolve) => setTimeout(resolve, 0)));
      });

      it("should not say trying again failed", () => {
        expect(screen.queryByText(/Trying again failed/)).toBeNull();
      });

      it("should keep the first result's copy", () => {
        expect(screen.getByText("These assets could not be read right now.")).toBeTruthy();
      });
    });
  });

  describe("when a card's View action is clicked", () => {
    let router: ReturnType<typeof show>["router"];

    beforeEach(async () => {
      net.fetchProfileAssets.mockResolvedValue(page([asset(1), name(5, "brai")]));
      ({ router } = show());
      await screen.findByRole("link", { name: "Power Up 1" });
      await user.click(screen.getByRole("link", { name: "View Power Up 1" }));
    });

    it("should open it in place, through the router", () => {
      expect(router.state.location.pathname).toBe("/shop/token/0xabc/1");
    });

    describe("and then a NAME's Transfer is clicked", () => {
      beforeEach(async () => {
        await user.click(screen.getByRole("link", { name: "Transfer brai" }));
      });

      it("should open it in place, through the router", () => {
        expect(router.state.location.pathname).toBe("/shop/token/0xens/5/transfer");
      });
    });
  });

  describe("when the read fails and the retry is loading", () => {
    beforeEach(async () => {
      net.fetchProfileAssets.mockRejectedValueOnce(new Error("503")).mockReturnValueOnce(new Promise(() => {}));
      show();
      await screen.findByText("Assets are unavailable right now.");
      await user.click(screen.getByRole("button", { name: "Try again" }));
    });

    it("should show the retry loading", async () => {
      expect(await screen.findByText("Loading assets")).toBeTruthy();
    });

    it("should not show the stale error", async () => {
      await screen.findByText("Loading assets");
      expect(screen.queryByText("Assets are unavailable right now.")).toBeNull();
    });
  });

  describe("when the owner has nothing", () => {
    beforeEach(() => {
      net.fetchProfileAssets.mockResolvedValue(page([]));
      show();
    });

    it("should say there are no assets yet", async () => {
      expect(await screen.findByText("No assets yet", { selector: "p" })).toBeTruthy();
    });

    it("should send the owner to the Shop", async () => {
      await screen.findByText("No assets yet", { selector: "p" });
      expect(screen.getByRole("link", { name: "Go to the Shop" }).getAttribute("href")).toBe("/shop");
    });
  });

  describe("when the read fails", () => {
    beforeEach(() => {
      net.fetchProfileAssets.mockRejectedValueOnce(new Error("503")).mockResolvedValueOnce(page([asset(1)]));
      show();
    });

    it("should say the read failed", async () => {
      expect(await screen.findByText("Assets are unavailable right now.")).toBeTruthy();
    });

    it("should never show an empty list", async () => {
      await screen.findByText("Assets are unavailable right now.");
      expect(screen.queryByText("No assets yet")).toBeNull();
    });

    describe("and Try again is clicked", () => {
      beforeEach(async () => {
        await screen.findByText("Assets are unavailable right now.");
        await user.click(screen.getByRole("button", { name: "Try again" }));
      });

      it("should show the list", async () => {
        expect(await screen.findByRole("link", { name: "Power Up 1" })).toBeTruthy();
      });
    });
  });

  describe("when a background re-read of a shown list fails", () => {
    let client: QueryClient;

    beforeEach(async () => {
      net.fetchProfileAssets.mockResolvedValueOnce(page([asset(1)])).mockRejectedValueOnce(new Error("503"));
      ({ client } = show());
      await screen.findByRole("link", { name: "Power Up 1" });
      await act(async () => {
        await client.refetchQueries({ queryKey: profileKeys.assets(A, null) });
        await new Promise((resolve) => setTimeout(resolve, 0));
      });
    });

    it("should record the error", () => {
      expect(client.getQueryState(profileKeys.assets(A, null))?.status).toBe("error");
    });

    it("should keep the list on screen", () => {
      expect(screen.getByRole("link", { name: "Power Up 1" })).toBeTruthy();
    });

    it("should not show the error state", () => {
      expect(screen.queryByText("Assets are unavailable right now.")).toBeNull();
    });
  });

  describe("when Load more is clicked", () => {
    beforeEach(async () => {
      const first = Array.from({ length: PROFILE_COLLECTIBLES_PAGE }, (_, n) => asset(n));
      net.fetchProfileAssets.mockResolvedValueOnce(page(first, 25)).mockResolvedValueOnce(page([asset(24)], 25));
      show();
      await screen.findByRole("link", { name: "Power Up 0" });
      await user.click(screen.getByRole("button", { name: "Load more" }));
    });

    it("should load the next page under the first", async () => {
      expect(await screen.findByRole("link", { name: "Power Up 24" })).toBeTruthy();
      expect(screen.getByRole("link", { name: "Power Up 0" })).toBeTruthy();
    });

    it("should no longer offer Load more", async () => {
      await screen.findByRole("link", { name: "Power Up 24" });
      expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
    });
  });
});
