
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { shopCart } from "../cart/store";
import { invalidateShopQueries } from "../discovery/query";
import { COLLECTION, ITEM_ID, RESALE_WEARABLE, RESELLER, ShopHarness, TOKEN_ID, catalogClient, catalogTransport, defaultAnswers, fixtureHost, type CatalogAnswers, type CatalogScenario, type SessionKind } from "../discovery/fixtures";
import type { TransportRequest } from "../api/signer";
import { encodeTokenId } from "./lib/token-id";
import { createShopClient, type ShopClient } from "../api";
import { BOOK } from "../payment/fixtures";
import { fixtureOfferSource, type OfferScenario } from "./fixtures";
import { ItemDetailRoute } from "./ItemDetail";
import { Route, Routes } from "react-router";
import { decodeFunctionData, type Hex } from "viem";
import { ERC20_ABI, LEGACY_MARKETPLACE, LEGACY_MARKETPLACE_ABI, manaToken } from "../market/lib/contracts";
import { fakeChain, fakeWallet } from "../market/testing";
import type { EvmAddress } from "../model/address";
import { CHAIN } from "../model/chain";

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response("[]", { status: 200, headers: { "content-type": "application/json" } })));
  invalidateShopQueries();
  shopCart.clear();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function itemPage(opts: { session?: SessionKind; scenario?: CatalogScenario; overrides?: CatalogAnswers; offers?: OfferScenario; itemId?: string; host?: ReturnType<typeof fixtureHost> } = {}) {
  return render(
    <ShopHarness
      path="/shop/item/:contractAddress/:itemId"
      entry={`/shop/item/${COLLECTION}/${opts.itemId ?? ITEM_ID}`}
      session={opts.session ?? "web3"}
      scenario={opts.scenario}
      overrides={opts.overrides}
      offers={opts.offers ? fixtureOfferSource(opts.offers) : undefined}
      host={opts.host}
    >
      <ItemDetailRoute />
    </ShopHarness>,
  );
}

function tokenPage(opts: { session?: SessionKind; offers?: OfferScenario; host?: ReturnType<typeof fixtureHost>; client?: ShopClient; tokenId?: string } = {}) {
  return render(
    <ShopHarness
      path="/shop/token/:contractAddress/:tokenId"
      entry={`/shop/token/${COLLECTION}/${opts.tokenId ?? TOKEN_ID}`}
      session={opts.session ?? "web3"}
      offers={opts.offers ? fixtureOfferSource(opts.offers) : undefined}
      host={opts.host}
      client={opts.client}
    >
      <ItemDetailRoute />
    </ShopHarness>,
  );
}

describe("the item route (a collection item, sold by its creator)", () => {
  it("loading: the preview and the info column hold skeletons", async () => {
    itemPage({ scenario: "loading" });
    await act(async () => {});
    expect(screen.getByTestId("item-detail")).toBeTruthy();
    expect(screen.getByLabelText("Loading preview").getAttribute("aria-busy")).toBe("true");
  });

  it("web3: the item, its stock, credits first and MANA second from the server offer", async () => {
    itemPage({ offers: { kind: "storeMint" } });
    expect(await screen.findByRole("heading", { level: 1, name: "Nacho Hat" })).toBeTruthy();
    expect(screen.getByTestId("detail-rarity-link").getAttribute("href")).toBe("/shop/items?rarities=epic");
    expect(await screen.findByTestId("detail-stock")).toHaveTextContent("Stock 10/1,000");
    expect(await screen.findByTestId("pay-credits")).toBeChecked();
    expect(screen.getByTestId("pay-mana")).not.toBeChecked();
    expect(screen.getByTestId("price-mana")).toHaveTextContent("2.3");
    expect(screen.getByTestId("detail-utility")).toHaveTextContent("Glows at night");
  });

  it("lists the item's resales as MANA-only offers, never with their feed credit figure", async () => {
    itemPage({ offers: { kind: "storeMint" } });
    const resales = await screen.findByTestId("resale-offers");
    expect(within(resales).getAllByTestId("resale-row")).toHaveLength(1);
    expect(within(resales).getByTestId("resale-price")).toHaveTextContent("MANA");
    expect(resales.textContent).not.toMatch(/Credit/);
    expect(within(resales).getByText("View").getAttribute("href")).toBe(`/shop/token/${COLLECTION}/${TOKEN_ID}`);
  });

  it("web2: credits only", async () => {
    itemPage({ session: "web2", offers: { kind: "storeMint" } });
    expect(await screen.findByTestId("cta-buy")).toHaveTextContent("Buy with Credits");
    expect(screen.queryByTestId("pay-mana")).toBeNull();
  });

  it("no quote route yet: the listing's credit price and the rule's choices, unavailable, with a retry", async () => {
    itemPage();
    const outage = await screen.findByTestId("payment-outage");
    expect(outage).toHaveTextContent("Checkout prices can't be confirmed right now");
    expect(screen.getByTestId("pay-credits")).toBeChecked();
    expect(screen.getByTestId("price-credits")).toHaveTextContent("12");
    expect(screen.getByTestId("cta-buy")).toBeDisabled();
  });

  it("a credits outage keeps credits as the main choice, unavailable, never switched to MANA", async () => {
    itemPage({ offers: { kind: "storeMint", outage: true } });
    expect(await screen.findByTestId("payment-outage")).toHaveTextContent("The Credits price is unavailable right now");
    expect(screen.getByTestId("pay-credits")).toBeChecked();
  });

  it("adding to the cart keeps the listing and the offer the buyer saw, and no chosen method", async () => {
    render(
      <ShopHarness path="/shop/*" entry={`/shop/item/${COLLECTION}/${ITEM_ID}`} session="web3" offers={fixtureOfferSource({ kind: "storeMint" })}>
        <Routes>
          <Route path="item/:contractAddress/:itemId" element={<ItemDetailRoute />} />
          <Route path="cart" element={<output data-testid="cart-page" />} />
        </Routes>
      </ShopHarness>,
    );
    fireEvent.click(await screen.findByTestId("cta-add-to-cart"));
    await waitFor(() => expect(shopCart.getSnapshot().lines).toHaveLength(1));
    const line = shopCart.getSnapshot().lines[0]!;
    expect(line.asset.token).toEqual({ kind: "item", itemId: ITEM_ID });
    expect(line.listing.tradeId).toBe("trade-primary-7");
    expect(line.method).toBeNull();
    expect(line.seen?.sale).toBe("primaryMint");
  });

  it("an unknown item is not available, with a way back", async () => {
    itemPage({ itemId: "404" });
    expect(await screen.findByTestId("item-not-found")).toHaveTextContent("This item isn't available");
  });

  it("a failed read says so and retries", async () => {
    itemPage({ scenario: "error" });
    const error = await screen.findByTestId("item-error");
    expect(error).toHaveTextContent("Couldn't load this item");
    expect(within(error).getByText("Try again")).toBeTruthy();
  });

  it("an index without a complete pass reads as catching up, not as missing", async () => {
    itemPage({ scenario: "outage" });
    expect(await screen.findByTestId("index-notice")).toHaveTextContent("the items mirror has no complete pass yet");
  });
});

describe("the description", () => {
  const LONG = "A hat for the discerning nacho, stitched by hand from the finest corn. ".repeat(3).trim();
  const answers: CatalogAnswers = { "/v2/catalog": { data: [{ data: { wearable: { description: LONG } } }], total: 1 } };

  afterEach(() => {
    vi.restoreAllMocks();
  });

  function layout(scrollHeight: number) {
    vi.stubGlobal(
      "ResizeObserver",
      class {
        cb: () => void;
        constructor(cb: () => void) {
          this.cb = cb;
        }
        observe() {
          this.cb();
        }
        disconnect() {}
      },
    );
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(120);
    vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(scrollHeight);
  }

  it("a long description that fits the four lines has no Read more", async () => {
    layout(120);
    itemPage({ overrides: answers });
    expect(await screen.findByText(LONG)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Read more" })).toBeNull();
  });

  it("a description the clamp cuts offers Read more, then Show less", async () => {
    layout(240);
    itemPage({ overrides: answers });
    fireEvent.click(await screen.findByRole("button", { name: "Read more" }));
    expect(screen.getByText(LONG)).toHaveAttribute("data-expanded");
    fireEvent.click(screen.getByRole("button", { name: "Show less" }));
    expect(screen.getByText(LONG)).not.toHaveAttribute("data-expanded");
  });
});

describe("the token route (one existing copy)", () => {
  it("web2 resale: MANA only with the wallet guidance and no credit wording", async () => {
    tokenPage({ session: "web2", offers: { kind: "resale" } });
    expect(await screen.findByRole("heading", { level: 1, name: "Nacho Hat #42" })).toBeTruthy();
    const panel = await screen.findByTestId("payment-panel");
    expect(panel).toHaveTextContent("This purchase is paid in MANA");
    expect(panel).toHaveTextContent("This is a resale");
    expect(panel.textContent).not.toMatch(/Credit/);
    expect(screen.queryByTestId("detail-stock")).toBeNull();
  });

  it("web3 resale: MANA is the only choice", async () => {
    tokenPage({ session: "web3", offers: { kind: "resale" } });
    expect(await screen.findByTestId("cta-buy")).toHaveTextContent("Buy with MANA");
    expect(screen.queryByTestId("pay-credits")).toBeNull();
  });

  it("with the NFT index down, the token's open resale from the catalogue is still for sale", async () => {
    const tokenId = encodeTokenId(ITEM_ID, 42n);
    const transport = catalogTransport("ok", {
      "/v3/catalog/unified": (url: URL) => (url.searchParams.get("itemId") === ITEM_ID ? { data: [{ ...RESALE_WEARABLE, tokenId }], total: 1 } : defaultAnswers()["/v3/catalog/unified"]!(url)),
    });
    const nftReads: string[] = [];
    const nftsDown = async (req: TransportRequest) => {
      if (!new URL(req.url, "http://shop.test").pathname.endsWith("/v1/nfts")) return transport(req);
      nftReads.push(req.url);
      return { status: 503, body: JSON.stringify({ error: "Service Unavailable" }) };
    };
    tokenPage({ tokenId, session: "web3", offers: { kind: "resale" }, client: createShopClient({ base: "http://shop.test", mode: "canonical", transport: nftsDown, signer: null }) });
    expect(await screen.findByTestId("cta-buy")).toHaveTextContent("Buy with MANA");
    expect(screen.queryByText("Not for sale")).toBeNull();
    expect(nftReads.length).toBeGreaterThan(0);
  });

  it("before the quote answers, a resale still shows MANA only", async () => {
    tokenPage({ session: "web3" });
    const panel = await screen.findByTestId("payment-panel");
    expect(panel.dataset.manaOnly).toBeDefined();
    expect(panel.textContent).not.toMatch(/Credit/);
  });

  it("the owner gets the manage actions instead of a purchase", async () => {
    tokenPage({ host: fixtureHost("web3", RESELLER) });
    const manage = await screen.findByTestId("manage-actions");
    expect(within(manage).getByText("Remove from sale").getAttribute("href")).toBe(`/shop/token/${COLLECTION}/${TOKEN_ID}/cancel`);
    expect(within(manage).getByText("Transfer").getAttribute("href")).toBe(`/shop/token/${COLLECTION}/${TOKEN_ID}/transfer`);
    expect(screen.queryByTestId("payment-panel")).toBeNull();
  });
});

describe("a LAND token", () => {
  const PARCEL = {
    nft: {
      id: `${BOOK.land}-${TOKEN_ID}`,
      activeOrderId: "order-1",
      category: "parcel",
      chainId: 1,
      contractAddress: BOOK.land,
      createdAt: 0,
      data: { parcel: { x: "12", y: "-4", description: null, estate: null } },
      image: "",
      issuedId: null,
      itemId: null,
      name: "Parcel 12,-4",
      network: "ETHEREUM",
      openRentalId: null,
      owner: RESELLER,
      tokenId: TOKEN_ID,
      soldAt: 0,
      updatedAt: 0,
      url: "",
    },
    order: { id: "order-1", marketplaceAddress: "0x8e5660b4ab70168b5a6feea0e0315cb49c8cd539", contractAddress: BOOK.land, tokenId: TOKEN_ID, owner: RESELLER, buyer: null, price: "4000000000000000000000", status: "open", expiresAt: 1_900_000_000_000, createdAt: 1_780_000_000_000, updatedAt: 1_780_000_000_000, network: "ETHEREUM", chainId: 1, issuedId: null, tradeId: null },
    rental: null,
  };

  it("shows the LAND sections and MANA only, with the web2 guidance and no credit wording", async () => {
    const nfts = (url: URL) => (url.searchParams.get("owner") && url.searchParams.get("owner") !== RESELLER ? { data: [], total: 0 } : { data: [PARCEL], total: 1 });
    const client = catalogClient("ok", { "/v1/nfts": nfts, "/v1/items": { data: [], total: 0 }, "/v3/catalog/unified": { data: [], total: 0 } });
    render(
      <ShopHarness path="/shop/token/:contractAddress/:tokenId" entry={`/shop/token/${BOOK.land}/${TOKEN_ID}`} session="web2" client={client} offers={fixtureOfferSource({ kind: "parcel" })}>
        <ItemDetailRoute />
      </ShopHarness>,
    );
    expect(await screen.findByRole("heading", { level: 1, name: "Parcel 12,-4" })).toBeTruthy();
    expect(await screen.findByTestId("land-branch")).toBeTruthy();
    const panel = await screen.findByTestId("payment-panel");
    expect(panel).toHaveTextContent("LAND, estates and rentals are sold in MANA");
    expect(panel.textContent).not.toMatch(/Credit/);
    expect(screen.queryByTestId("resale-offers")).toBeNull();
  });
});

describe("a LAND legacy order, bought from its token page", () => {
  const ORDER = { id: "order-1", marketplaceAddress: LEGACY_MARKETPLACE[CHAIN.ETHEREUM]!.address, contractAddress: BOOK.land, tokenId: TOKEN_ID, owner: RESELLER, buyer: null, price: "4000000000000000000000", status: "open", expiresAt: 1_900_000_000_000, createdAt: 1_780_000_000_000, updatedAt: 1_780_000_000_000, network: "ETHEREUM", chainId: 1, issuedId: null, tradeId: null };
  const PARCEL_ROW = {
    nft: { id: `${BOOK.land}-${TOKEN_ID}`, activeOrderId: "order-1", category: "parcel", chainId: 1, contractAddress: BOOK.land, createdAt: 0, data: { parcel: { x: "12", y: "-4", description: null, estate: null } }, image: "", issuedId: null, itemId: null, name: "Parcel 12,-4", network: "ETHEREUM", openRentalId: null, owner: RESELLER, tokenId: TOKEN_ID, soldAt: 0, updatedAt: 0, url: "" },
    order: ORDER,
    rental: null,
  };

  it("web3: approves the legacy marketplace for the order's exact MANA, then executes the order; never the cart", async () => {
    const wallet = fakeWallet({ chainId: 1 });
    const restore = fakeChain({ contracts: { [manaToken(CHAIN.ETHEREUM).address]: { abi: ERC20_ABI, answer: () => 0n } } });
    try {
      const nfts = (url: URL) => (url.searchParams.get("owner") && url.searchParams.get("owner") !== RESELLER ? { data: [], total: 0 } : { data: [PARCEL_ROW], total: 1 });
      const client = catalogClient("ok", { "/v1/nfts": nfts, "/v1/items": { data: [], total: 0 }, "/v3/catalog/unified": { data: [], total: 0 } });
      const host = fixtureHost("web3", wallet.address as EvmAddress, { provider: () => wallet });
      render(
        <ShopHarness path="/shop/token/:contractAddress/:tokenId" entry={`/shop/token/${BOOK.land}/${TOKEN_ID}`} session="web3" host={host} client={client} offers={fixtureOfferSource({ kind: "parcel" })}>
          <ItemDetailRoute />
        </ShopHarness>,
      );
      const buy = await screen.findByTestId("cta-buy");
      await waitFor(() => expect(buy).toBeEnabled());
      fireEvent.click(buy);
      const dialog = await screen.findByTestId("legacy-order-buy");
      expect(within(dialog).getByTestId("order-price")).toHaveTextContent("4,000 MANA");
      expect(dialog.textContent).not.toMatch(/Credit/);
      const confirm = within(dialog).getByTestId("order-buy");
      await waitFor(() => expect(confirm).toBeEnabled());
      await act(async () => fireEvent.click(confirm));
      expect(await within(dialog).findByText(/Purchase complete/)).toBeTruthy();
      const calls = wallet.sent.map((tx) => [tx.to.toLowerCase(), decodeFunctionData({ abi: [...ERC20_ABI, ...LEGACY_MARKETPLACE_ABI], data: tx.data as Hex })]);
      expect(calls.map(([to, d]) => [to, (d as { functionName: string }).functionName])).toEqual([
        [manaToken(CHAIN.ETHEREUM).address.toLowerCase(), "approve"],
        [LEGACY_MARKETPLACE[CHAIN.ETHEREUM]!.address.toLowerCase(), "executeOrder"],
      ]);
      expect((calls[0]![1] as { args: readonly unknown[] }).args[1]).toBe(4000n * 10n ** 18n);
      expect((calls[1]![1] as { args: readonly unknown[] }).args).toEqual([BOOK.land, BigInt(TOKEN_ID), 4000n * 10n ** 18n]);
      expect(shopCart.getSnapshot().lines).toEqual([]);
    } finally {
      restore();
    }
  });
});

describe("the seed a card passes", () => {
  it("renders the passed item at once and pins identity to the route", async () => {
    await act(async () => {
      itemPage({ offers: { kind: "creatorOrder" } });
    });
    expect(screen.getByTestId("item-detail").dataset.route).toBe("item");
  });
});
