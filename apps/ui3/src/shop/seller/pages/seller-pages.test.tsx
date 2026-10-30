import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { CHAIN } from "../../model/chain";
import { BOOK } from "../../payment/fixtures";
import { MANA, sale } from "../../market/fixtures";
import { COLLECTION_V2_ABI, ERC721_ABI, latestOffChainMarketplace, MARKETPLACE_ABI, COUPON_MANAGER_ABI, COUPON_MANAGERS } from "../../market/lib/contracts";
import { fakeChain, fakeWallet, type FakeRoute } from "../../market/testing";
import { mountShop, StoryShell } from "../../market/testHarness";
import { CreatorSaleForm, salePrice } from "../components/CreatorSales";
import { IssueForm } from "../components/IssueModal";
import { ImportListingsView } from "../components/ImportListings";
import type { PublishableItem } from "../lib/builder";
import type { ImportItem } from "../lib/import";
import { setShopCapabilities } from "../lib/featureFlags";

const MARKET = latestOffChainMarketplace(CHAIN.POLYGON).address;
const originalFetch = globalThis.fetch;
let restoreChain: (() => void) | null = null;
let deployed: FormData[] = [];
let builderCalls: string[] = [];

const COLLECTION = { id: "col-1", name: "Neon Nights", eth_address: "", contract_address: BOOK.collection, is_published: true, is_approved: true };
const ITEMS = [
  { id: "i7", collection_id: "col-1", blockchain_item_id: "7", name: "Neon Jacket", rarity: "legendary", total_supply: 40, is_published: true, is_approved: true },
  { id: "i8", collection_id: "col-1", blockchain_item_id: "8", name: "Cyber Boots", rarity: "epic", total_supply: 10, is_published: true, is_approved: true },
];

beforeEach(() => {
  deployed = [];
  builderCalls = [];
  localStorage.clear();
  globalThis.fetch = (async (url: string, init?: RequestInit) => {
    const u = new URL(String(url), "http://x");
    if (u.pathname.endsWith("/content/entities/active"))
      return Response.json([{ content: [], metadata: { description: "Neon things", images: [], links: [{ name: "twitter", url: "https://www.twitter.com/neon" }] } }]);
    if (u.pathname.endsWith("/content/entities")) {
      deployed.push(init!.body as FormData);
      return new Response("{}");
    }
    if (u.pathname.includes("/builder-api/v1/")) {
      builderCalls.push(u.pathname);
      if (u.pathname.endsWith("/collections")) return Response.json({ data: [COLLECTION] });
      return Response.json({ data: ITEMS });
    }
    if (u.pathname.endsWith("/transactions")) return Response.json({ ok: true, txHash: `0x${"e".repeat(64)}` });
    throw new Error(`unexpected fetch ${u.pathname}`);
  }) as typeof fetch;
  restoreChain = fakeChain({
    contracts: {
      [MARKET]: { abi: MARKETPLACE_ABI, answer: () => 0n },
      [BOOK.collection]: { abi: [...ERC721_ABI, ...COLLECTION_V2_ABI], answer: (fn) => (fn === "globalMinters" || fn === "isApprovedForAll" ? true : 0n) },
      [COUPON_MANAGERS.v3[CHAIN.POLYGON]!.address]: { abi: COUPON_MANAGER_ABI, answer: () => 0n },
    },
  });
});

afterEach(() => {
  restoreChain?.();
  globalThis.fetch = originalFetch;
  act(() => setShopCapabilities(null));
});

describe("Store settings: the owner's own store", () => {
  it("signed out: the sign-in gate", async () => {
    mountShop({ path: "/shop/store-settings", who: "signedOut" });
    expect(await screen.findByTestId("shop-signin-gate")).toHaveTextContent("Sign in to set up your store");
  });

  it("loads the saved store, validates links, and deploys the edited entity signed by the owner", async () => {
    const wallet = fakeWallet();
    mountShop({ path: "/shop/store-settings", who: "web2", wallet });
    const description = await screen.findByLabelText("Description");
    expect(description).toHaveValue("Neon things");
    expect(screen.getByLabelText("Twitter handle")).toHaveValue("neon");
    fireEvent.change(screen.getByLabelText("Website"), { target: { value: "http://insecure.example" } });
    expect(screen.getByText("Link should start with https://")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Website"), { target: { value: "https://neon.example" } });
    fireEvent.change(description, { target: { value: "Neon things, since 2021" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(deployed).toHaveLength(1));
    const form = deployed[0]!;
    expect(form.get("entityId")).toMatch(/^bafkrei/);
    const entityFile = [...form.entries()].find(([key]) => key === form.get("entityId"))?.[1] as Blob;
    const entity = JSON.parse(await entityFile.text()) as { type: string; pointers: string[]; metadata: { owner: string; description: string; links: { name: string; url: string }[] } };
    expect(entity.type).toBe("store");
    expect(entity.pointers).toEqual([`urn:decentraland:off-chain:marketplace-stores:${wallet.address}`]);
    expect(entity.metadata).toMatchObject({ owner: wallet.address, description: "Neon things, since 2021" });
    expect(entity.metadata.links.map((l) => l.name)).toEqual(["website", "twitter"]);
  });
});

const summary = { total: 3, mints: 2, resales: 1, earnedWei: MANA(10), byCollection: [{ contractAddress: BOOK.collection, sold: 2, earnedWei: MANA(6) }], byItem: [], royalties: { resales: 4, volumeWei: MANA(40) } };
const storeRoutes = (over: FakeRoute[] = []): FakeRoute[] => [
  ...over,
  { method: "GET", path: /^\/v1\/sales\/summary$/, reply: () => ({ body: { data: summary } }) },
  { method: "GET", path: /^\/v1\/sales$/, reply: () => ({ body: { data: [sale({ id: "s1" }), sale({ id: "s2", itemId: "8" })], total: 2 } }) },
  { method: "GET", path: /^\/v3\/catalog\/shop$/, reply: () => ({ body: { data: [{ listingType: "primary", itemId: "7", contractAddress: BOOK.collection, priceCredits: 25, tradeId: "t7", compareAtCredits: null, saleEndsAt: null }], total: 1 } }) },
  { method: "GET", path: /^\/v3\/catalog\/items$/, reply: () => ({ body: { data: [{ itemId: "7", contractAddress: BOOK.collection, isOnSale: true, price: "1" }], total: 1 } }) },
  { method: "GET", path: /^\/v1\/picks\/stats$/, reply: () => ({ body: { ok: true, data: [] } }) },
  { method: "GET", path: /^\/v3\/catalog\/importable$/, reply: () => ({ body: { data: [] } }) },
];

describe("My store", () => {
  it("shows sales and MANA earnings, the collections with each item's Shop state, and lists an unlisted item for credits", async () => {
    const wallet = fakeWallet();
    const posted: unknown[] = [];
    mountShop({
      path: "/shop/my-store",
      who: "web3",
      wallet,
      routes: storeRoutes([
        { method: "GET", path: /^\/v1\/coupons$/, reply: () => ({ status: 503, body: { error: "Service Unavailable", message: "coupons index", source: "coupons" } }) },
        { method: "POST", path: /^\/v1\/trades$/, reply: ({ body }) => (posted.push(body), { status: 201, body: { ok: true, data: { ...(body as object), id: "p-1", contract: MARKET } } }) },
      ]),
    });
    expect(await screen.findByTestId("store-tile-sold")).toHaveTextContent("3");
    expect(screen.getByTestId("store-tile-earnings")).toHaveTextContent("10");
    expect(screen.getByTestId("store-tile-earnings")).toHaveTextContent("Paid to you this period");
    expect(builderCalls[0]).toBe(`/builder-api/v1/${wallet.address}/collections`);
    fireEvent.click(screen.getByRole("button", { name: "Neon Nights" }));
    const items = await screen.findAllByTestId("store-item");
    expect(within(items.find((i) => i.textContent?.includes("Neon Jacket"))!).getByTestId("store-item-state")).toHaveTextContent("On sale for 25 credits");
    const boots = items.find((i) => i.textContent?.includes("Cyber Boots"))!;
    expect(within(boots).getByTestId("store-item-state")).toHaveTextContent("Not listed");
    fireEvent.click(within(boots).getByRole("button", { name: "Put on sale" }));
    fireEvent.change(await screen.findByPlaceholderText("10"), { target: { value: "15" } });
    expect(screen.getByText(/\$1\.50/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Put on sale" }));
    expect(await screen.findByTestId("shop-primary-listed")).toHaveTextContent("15 credits");
    expect(posted[0]).toMatchObject({ type: "public_item_order", sent: [{ assetType: 4, itemId: "8" }], received: [{ assetType: 2, amount: "1500000000000000000", beneficiary: wallet.address }] });
    expect((posted[0] as { checks: { uses: number } }).checks.uses).toBe(990);
    expect(await screen.findByTestId("shop-outage")).toHaveTextContent("the coupons index is still catching up");
    expect(await screen.findAllByTestId("store-sale-row")).toHaveLength(2);
  });

  it("says it is loading while the catalogue and the sales are in flight, and the store settings too", async () => {
    const before = globalThis.fetch;
    globalThis.fetch = ((url: string, init?: RequestInit) =>
      String(url).includes("/builder-api/") || String(url).includes("/content/entities/active") ? new Promise<Response>(() => {}) : before(url, init)) as typeof fetch;
    const { PENDING } = await import("../../market/testing");
    const view = mountShop({ path: "/shop/my-store", who: "web3", routes: [{ method: "GET", path: /^\/v1\/sales/, reply: PENDING }, { method: "GET", path: /./, reply: PENDING }] });
    expect(await screen.findByText("Loading your store…")).toBeInTheDocument();
    view.unmount();
    mountShop({ path: "/shop/store-settings", who: "web3" });
    expect(await screen.findByText("Loading your store…")).toBeInTheDocument();
  });

  it("an old cancel link naming only the order opens that listing's cancellation, or says it is no longer open", async () => {
    const wallet = fakeWallet();
    const order = { id: "o-12", marketplaceAddress: MARKET, contractAddress: BOOK.collection, tokenId: "105312", owner: wallet.address, buyer: null, price: MANA(3), status: "open", expiresAt: 0, createdAt: 0, updatedAt: 0, network: "MATIC", chainId: 137, issuedId: null, tradeId: "t-1" };
    const { PENDING } = await import("../../market/testing");
    const view = mountShop({
      path: "/shop/my-store?order=o-12",
      who: "web3",
      wallet,
      routes: [{ method: "GET", path: /^\/v1\/orders$/, reply: ({ url }) => ({ body: { data: url.searchParams.get("owner") === wallet.address ? [order] : [], total: 1 } }) }, { method: "GET", path: /./, reply: PENDING }],
    });
    expect(await screen.findByText("Loading the item…")).toBeInTheDocument();
    expect(screen.queryByTestId("shop-order-from-link")).toBeNull();
    view.unmount();
    mountShop({ path: "/shop/my-store?order=o-99", who: "web3", wallet, routes: [{ method: "GET", path: /^\/v1\/orders$/, reply: () => ({ body: { data: [order], total: 1 } }) }, { method: "GET", path: /./, reply: PENDING }] });
    expect(await screen.findByTestId("shop-order-from-link")).toHaveTextContent("The listing from your link is no longer open");
  });

  it("signed out: the gate; a store dashboard narrowed to other addresses sends the visitor to the overview", async () => {
    const view = mountShop({ path: "/shop/my-store", who: "signedOut" });
    expect(await screen.findByTestId("shop-signin-gate")).toHaveTextContent("Sign in to view your store");
    view.unmount();
    setShopCapabilities({ myStoreOnly: [BOOK.creator] });
    const { PENDING } = await import("../../market/testing");
    mountShop({ path: "/shop/my-store", who: "web3", routes: [{ method: "GET", path: /./, reply: PENDING }] });
    expect(await screen.findByTestId("overview")).toBeInTheDocument();
    expect(screen.queryByTestId("shop-signin-gate")).toBeNull();
  });
});

async function renderAlone(node: React.ReactNode, who: "web2" | "web3" = "web3") {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const view = render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <StoryShell who={who}>{node}</StoryShell>
      </MemoryRouter>
    </QueryClientProvider>,
  );
  await act(async () => {});
  return view;
}

const publishable: PublishableItem = {
  id: "i8",
  collectionId: "col-1",
  collectionName: "Neon Nights",
  contractAddress: BOOK.collection,
  blockchainItemId: "8",
  name: "Cyber Boots",
  category: "feet",
  rarity: "epic",
  thumbnail: "",
  type: "wearable",
  isPublished: true,
  isApproved: true,
  totalSupply: 10,
  maxSupply: 1000,
  remainingSupply: 3,
  minters: [],
};

describe("Discounts, issuance and the listing import", () => {
  it("previews a discount in whole credits and refuses to publish on a node without its coupons index", async () => {
    const { setShopClient } = await import("../../api/shop");
    const { fakeShop } = await import("../../market/testing");
    setShopClient(fakeShop([{ method: "POST", path: /^\/v1\/coupons$/, reply: () => ({ status: 503, body: { error: "Service Unavailable", message: "x", source: "coupons" } }) }]).client);
    try {
      expect(salePrice(10, 20)).toBe(8);
      expect(salePrice(1, 20)).toBe(1);
      await renderAlone(
        <CreatorSaleForm
          collection={{ contractAddress: BOOK.collection, name: "Neon Nights", listedCount: 1, examplePriceCredits: 1, items: [{ key: "k", name: "x", thumbnail: "", priceCredits: 1, state: "discounted", remainingSupply: 1 }] }}
          onClose={() => {}}
        />,
      );
      expect(screen.getByTestId("shop-sale-preview")).toHaveTextContent("would still sell for the same price");
      fireEvent.click(screen.getByRole("button", { name: "50%" }));
      fireEvent.click(screen.getByRole("button", { name: "Review discount" }));
      fireEvent.click(screen.getByRole("button", { name: "Start discount" }));
      expect(await screen.findByRole("alert")).toHaveTextContent("Discounts are not available on this node yet");
    } finally {
      setShopClient(null);
    }
  });

  it("issuing copies checks addresses and the remaining supply before any prompt", async () => {
    await renderAlone(<IssueForm item={publishable} onClose={() => {}} />);
    const address = screen.getByPlaceholderText("0x…");
    fireEvent.change(address, { target: { value: "nope" } });
    expect(screen.getByText("Enter a valid address (0x…).")).toBeInTheDocument();
    fireEvent.change(address, { target: { value: BOOK.reseller } });
    fireEvent.change(screen.getByLabelText("Amount"), { target: { value: "4" } });
    expect(screen.getByText("You can issue at most 3 more copies of this item.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Issue 4 copies" })).toBeDisabled();
  });

  it("the import tool lists creations for credits and shows owned resales staying in MANA, never importable", async () => {
    const creation: ImportItem = { oldTradeId: "o1", listingType: "primary", contractAddress: BOOK.collection, itemId: "7", tokenId: null, name: "Neon Jacket", thumbnail: "", rarity: "rare", category: "wearable", wearableCategory: null, manaWei: MANA(3), available: 25, network: "MATIC", chainId: 137, suggestedCredits: 15 };
    const owned: ImportItem = { ...creation, oldTradeId: "o2", listingType: "secondary", itemId: null, tokenId: "42", name: "Resold Visor", suggestedCredits: null };
    await renderAlone(<ImportListingsView creations={[creation]} owned={[owned]} />);
    expect(screen.getAllByTestId("shop-import-row")).toHaveLength(1);
    expect(screen.getByLabelText("Include Neon Jacket")).toBeChecked();
    const ownedPanel = screen.getByTestId("shop-import-owned");
    expect(ownedPanel).toHaveTextContent("Resales stay in MANA");
    expect(within(ownedPanel).queryByRole("checkbox")).toBeNull();
    expect(within(ownedPanel).getByRole("link", { name: "Change price" })).toHaveAttribute("href", `/shop/token/${BOOK.collection}/42/sell`);
    fireEvent.change(screen.getByPlaceholderText("Credits"), { target: { value: "0" } });
    expect(screen.getByRole("button", { name: "List 1 item" })).toBeDisabled();
  });
});
