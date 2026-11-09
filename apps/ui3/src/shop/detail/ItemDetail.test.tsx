
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, onTestFinished, vi } from "vitest";

import { shopCart } from "../cart/store";
import { invalidateShopQueries } from "../discovery/query";
import { COLLECTION, ITEM_ID, RESALE_WEARABLE, RESELLER, ShopHarness, TOKEN_ID, catalogClient, catalogTransport, defaultAnswers, fixtureHost, type CatalogAnswers, type CatalogScenario, type SessionKind } from "../discovery/fixtures";
import type { TransportRequest } from "../api/signer";
import { encodeTokenId } from "./lib/token-id";
import { createShopClient, type ShopClient } from "../api";
import { BOOK } from "../payment/fixtures";
import { fixtureOfferSource, type OfferScenario } from "./fixtures";
import { ItemDetailRoute } from "./ItemDetail";
import { stashBuyNowReturn, takeBuyNowReturn } from "./buyNowReturn";
import { Route, Routes } from "react-router";
import { decodeFunctionData, type Hex } from "viem";
import { ERC20_ABI, LEGACY_MARKETPLACE, LEGACY_MARKETPLACE_ABI, manaToken } from "../market/lib/contracts";
import { fakeChain, fakeWallet } from "../market/testing";
import { marketplaceOn } from "../market/testing-flags";
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

function itemPage(opts: { session?: SessionKind; scenario?: CatalogScenario; overrides?: CatalogAnswers; offers?: OfferScenario; itemId?: string; host?: ReturnType<typeof fixtureHost>; remoteFlags?: Record<string, boolean> | null; search?: string } = {}) {
  return render(
    <ShopHarness
      path="/shop/item/:contractAddress/:itemId"
      entry={`/shop/item/${COLLECTION}/${opts.itemId ?? ITEM_ID}${opts.search ?? ""}`}
      session={opts.session ?? "web3"}
      scenario={opts.scenario}
      overrides={opts.overrides}
      offers={opts.offers ? fixtureOfferSource(opts.offers) : undefined}
      host={opts.host}
      remoteFlags={opts.remoteFlags}
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
    expect(document.querySelector(".shop-pdp[data-route]")).toBeTruthy();
    expect(screen.getByLabelText("Loading preview").getAttribute("aria-busy")).toBe("true");
  });

  it("web3: the item, its stock and upstream's two CTAs; how to pay is Buy now's dialog, never a choice on the page", async () => {
    itemPage({ offers: { kind: "storeMint" } });
    expect(await screen.findByRole("heading", { level: 1, name: "Nacho Hat" })).toBeTruthy();
    expect(screen.getByTestId("detail-rarity-link").getAttribute("href")).toBe("/shop/items?rarities=epic");
    expect(await screen.findByTestId("detail-stock")).toHaveTextContent("Stock 10/1,000");
    expect(screen.getByTestId("detail-utility")).toHaveTextContent("Glows at night");
    expect(await screen.findByTestId("cta-buy-now")).toHaveTextContent("Buy now");
    expect(screen.getByTestId("cta-add-to-cart")).toBeTruthy();
    expect(screen.queryByTestId("pay-credits")).toBeNull();
    expect(screen.queryByTestId("pay-mana")).toBeNull();
    fireEvent.click(screen.getByTestId("cta-buy-now"));
    const dialog = await screen.findByTestId("buy-modal");
    expect(dialog).toHaveAttribute("aria-label", "Buy Nacho Hat");
    await waitFor(() => expect(dialog.querySelector("[data-phase]")?.getAttribute("data-phase")).not.toBe("loading"));
    expect(within(dialog).getByTestId("buy-modal-asset")).toHaveTextContent("Nacho Hat");
    fireEvent.click(within(dialog).getByTestId("buy-modal-close"));
    expect(screen.queryByTestId("buy-modal")).toBeNull();
  });

  it("lists the item's resales as MANA-only offers, never with their feed credit figure", async () => {
    onTestFinished(marketplaceOn());
    itemPage({ offers: { kind: "storeMint" } });
    const resales = await screen.findByTestId("resale-offers");
    expect(within(resales).getAllByTestId("resale-row")).toHaveLength(1);
    expect(within(resales).getByTestId("resale-price")).toHaveTextContent("MANA");
    expect(resales.textContent).not.toMatch(/Credit/);
    expect(within(resales).getByText("View").getAttribute("href")).toBe(`/shop/token/${COLLECTION}/${TOKEN_ID}`);
  });

  it("an item whose only listing is a resale is not for sale here without the Marketplace, and never asks for it", async () => {
    const asked: (string | null)[] = [];
    const resaleOnly: CatalogAnswers = {
      "/v3/catalog/unified": (url: URL) => {
        const listingType = url.searchParams.get("listingType");
        asked.push(listingType);
        const rows = listingType && listingType !== "secondary" ? [] : [RESALE_WEARABLE];
        return { data: rows, total: rows.length };
      },
    };
    for (const session of ["web2", "web3"] as const) {
      asked.length = 0;
      invalidateShopQueries();
      const view = itemPage({ session, overrides: resaleOnly, offers: { kind: "resale" } });
      expect(await screen.findByText("Not for sale")).toBeTruthy();
      expect(screen.queryByTestId("price-mana")).toBeNull();
      expect(asked.length).toBeGreaterThan(0);
      expect(asked.every((t) => t === "primary")).toBe(true);
      view.unmount();
    }

    // A web3 session with the rollout on sees the resale as the item's offer.
    onTestFinished(marketplaceOn());
    invalidateShopQueries();
    itemPage({ session: "web3", overrides: resaleOnly, offers: { kind: "resale" } });
    expect(await screen.findByTestId("price-mana")).toBeTruthy();
    expect(screen.queryByText("Not for sale")).toBeNull();
  });

  it("a web3 visitor keeps the creator's price on screen while the rollout answers and the wider read lands", async () => {
    let answerFlags = (_on: boolean): void => undefined;
    const flags = new Promise<Response>((resolve) => (answerFlags = (on) => resolve(Response.json({ flags: { "dapps-shop-marketplace-features": on }, variants: {} }))));
    vi.stubGlobal("fetch", vi.fn((input: RequestInfo | URL) => (String(input).endsWith("/dapps.json") ? flags : Promise.resolve(new Response("[]", { status: 200 })))));
    let release = (): void => undefined;
    const held = new Promise<void>((r) => (release = r));
    const wider: (string | null)[] = [];
    const answers = defaultAnswers();
    const listing: CatalogAnswers = {
      "/v3/catalog/unified": async (url: URL) => {
        if (url.searchParams.get("itemId") && !url.searchParams.get("listingType")) {
          wider.push(url.searchParams.get("itemId"));
          await held;
        }
        return answers["/v3/catalog/unified"]!(url);
      },
    };
    itemPage({ session: "web3", overrides: listing, offers: { kind: "storeMint" }, remoteFlags: null });
    expect(await screen.findByTestId("detail-stock")).toHaveTextContent("Stock 10/1,000");
    await act(async () => answerFlags(true));
    await waitFor(() => expect(wider.length).toBeGreaterThan(0));
    // The wider read is still out: the primary answer stays, never "Not for sale".
    expect(screen.queryByText("Not for sale")).toBeNull();
    expect(screen.getByTestId("detail-stock")).toHaveTextContent("Stock 10/1,000");
    await act(async () => release());
    expect(await screen.findByTestId("resale-offers")).toBeTruthy();
  });

  it("the related rail asks for primary sales only without the Marketplace", async () => {
    const asked: (string | null)[] = [];
    const related: CatalogAnswers = {
      "/v3/catalog/related": (url: URL) => {
        asked.push(url.searchParams.get("listingType"));
        return { data: [] };
      },
    };
    const view = itemPage({ session: "web2", overrides: related, offers: { kind: "storeMint" } });
    await waitFor(() => expect(asked.length).toBeGreaterThan(0));
    expect(asked.every((t) => t === "primary")).toBe(true);
    view.unmount();

    onTestFinished(marketplaceOn());
    asked.length = 0;
    invalidateShopQueries();
    itemPage({ session: "web3", overrides: related, offers: { kind: "storeMint" } });
    await waitFor(() => expect(asked).toContain(null));
  });

  /** Opens Buy now's dialog and waits for it to settle on a screen. */
  async function openBuyNow() {
    fireEvent.click(await screen.findByTestId("cta-buy-now"));
    const dialog = await screen.findByTestId("buy-modal");
    await waitFor(() => expect(dialog.querySelector("[data-phase]")?.getAttribute("data-phase")).not.toBe("loading"));
    return dialog;
  }

  it("web2: credits only, in Buy now's dialog too: no MANA to choose", async () => {
    itemPage({ session: "web2", offers: { kind: "storeMint" } });
    expect(await screen.findByTestId("cta-buy-now")).toHaveTextContent("Buy now");
    const dialog = await openBuyNow();
    expect(within(dialog).queryByTestId("buy-modal-methods")).toBeNull();
    expect(within(dialog).queryByTestId("pay-mana")).toBeNull();
  });

  it("no quote route yet: the page keeps the listing's credit price, and Buy now says the price can't be checked, with a retry", async () => {
    itemPage();
    expect(await screen.findByTestId("item-price")).toHaveTextContent("12");
    const dialog = await openBuyNow();
    expect(within(dialog).getByTestId("buy-error")).toHaveTextContent("Prices can't be checked right now");
    expect(within(dialog).getByRole("button", { name: "Try again" })).toBeTruthy();
    expect(within(dialog).queryByTestId("buy-modal-buy")).toBeNull();
  });

  it("a credits outage never switches to MANA: Buy now says the price can't be checked", async () => {
    itemPage({ offers: { kind: "storeMint", outage: true } });
    const dialog = await openBuyNow();
    expect(within(dialog).getByTestId("buy-error")).toHaveTextContent("Prices can't be checked right now");
    expect(within(dialog).queryByTestId("buy-modal-buy")).toBeNull();
  });

  it("?buy=resume alone, as any link can carry it, opens nothing", async () => {
    sessionStorage.clear();
    itemPage({ offers: { kind: "storeMint" }, search: "?buy=resume" });
    expect(await screen.findByTestId("cta-buy-now")).toBeTruthy();
    await act(async () => {});
    expect(screen.queryByTestId("buy-modal")).toBeNull();
  });

  it("?buy=resume with this tab's record of the purchase the buyer confirmed reopens Buy now's dialog, once", async () => {
    sessionStorage.clear();
    stashBuyNowReturn({ path: `/shop/item/${COLLECTION}/${ITEM_ID}`, lineId: "the-line", credits: 12 });
    itemPage({ offers: { kind: "storeMint" }, search: "?buy=resume" });
    expect(await screen.findByTestId("buy-modal")).toBeTruthy();
    expect(takeBuyNowReturn(`/shop/item/${COLLECTION}/${ITEM_ID}`)).toBeNull();
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
  beforeEach(() => marketplaceOn());
  it("a web2 session sees the token but no resale to buy: resales are the Marketplace's", async () => {
    tokenPage({ session: "web2", offers: { kind: "resale" } });
    expect(await screen.findByRole("heading", { level: 1, name: "Nacho Hat #42" })).toBeTruthy();
    expect(screen.queryByTestId("payment-panel")).toBeNull();
    expect(screen.queryByTestId("cta-buy")).toBeNull();
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
  beforeEach(() => marketplaceOn());
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

  it("shows the LAND sections and MANA only, with no credit wording, to a web3 session", async () => {
    const nfts = (url: URL) => (url.searchParams.get("owner") && url.searchParams.get("owner") !== RESELLER ? { data: [], total: 0 } : { data: [PARCEL], total: 1 });
    const client = catalogClient("ok", { "/v1/nfts": nfts, "/v1/items": { data: [], total: 0 }, "/v3/catalog/unified": { data: [], total: 0 } });
    render(
      <ShopHarness path="/shop/token/:contractAddress/:tokenId" entry={`/shop/token/${BOOK.land}/${TOKEN_ID}`} session="web3" client={client} offers={fixtureOfferSource({ kind: "parcel" })}>
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

  it("a web2 session sees the LAND token but none of the Marketplace: no LAND sections, nothing for sale", async () => {
    const nfts = (url: URL) => (url.searchParams.get("owner") && url.searchParams.get("owner") !== RESELLER ? { data: [], total: 0 } : { data: [PARCEL], total: 1 });
    const client = catalogClient("ok", { "/v1/nfts": nfts, "/v1/items": { data: [], total: 0 }, "/v3/catalog/unified": { data: [], total: 0 } });
    render(
      <ShopHarness path="/shop/token/:contractAddress/:tokenId" entry={`/shop/token/${BOOK.land}/${TOKEN_ID}`} session="web2" client={client} offers={fixtureOfferSource({ kind: "parcel" })}>
        <ItemDetailRoute />
      </ShopHarness>,
    );
    expect(await screen.findByRole("heading", { level: 1, name: "Parcel 12,-4" })).toBeTruthy();
    expect(screen.queryByTestId("land-branch")).toBeNull();
    expect(screen.queryByTestId("payment-panel")).toBeNull();
  });
});

describe("a LAND legacy order, bought from its token page", () => {
  beforeEach(() => marketplaceOn());
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
    expect(document.querySelector<HTMLElement>(".shop-pdp[data-route]")!.dataset.route).toBe("item");
  });
});

describe("latest sales and owners", () => {
  beforeEach(() => marketplaceOn());
  const BUYER = "0x00000000000000000000000000000000000000b1";
  const sale = (n: number) => ({
    id: `sale-${n}`,
    itemId: ITEM_ID,
    contractAddress: COLLECTION,
    buyer: BUYER,
    chainId: 137,
    network: "MATIC",
    price: "2500000000000000000",
    seller: RESELLER,
    timestamp: Date.now() - 30 * 86_400_000,
    tokenId: TOKEN_ID,
    txHash: `0x${n}`,
    type: n === 0 ? "mint" : "order",
  });
  const owner = (n: number) => ({ issuedId: String(n + 1), ownerId: BUYER, tokenId: String(1000 + n) });

  function answers(seen: { sales: URL[]; owners: URL[] }, totals = { sales: 2, owners: 2 }): CatalogAnswers {
    return {
      "/v1/sales": (url: URL) => {
        seen.sales.push(url);
        const skip = Number(url.searchParams.get("skip") ?? 0);
        const rows = Array.from({ length: Math.max(0, Math.min(5, totals.sales - skip)) }, (_, i) => sale(skip + i));
        return { data: rows, total: totals.sales };
      },
      "/v1/owners": (url: URL) => {
        seen.owners.push(url);
        return { data: Array.from({ length: Math.min(5, totals.owners) }, (_, i) => owner(i)), total: totals.owners };
      },
    };
  }

  it("the item route lists the item's owners by issue number and the sales of all its tokens", async () => {
    const seen = { sales: [] as URL[], owners: [] as URL[] };
    itemPage({ overrides: answers(seen) });
    const owners = await screen.findByTestId("item-owners");
    expect(within(owners).getAllByTestId("owner-row")).toHaveLength(2);
    expect(within(owners).getByText("2 copies")).toBeTruthy();
    expect(within(owners).getAllByText("View")[0]!.getAttribute("href")).toBe(`/shop/token/${COLLECTION}/1000`);
    expect(seen.owners[0]!.searchParams.get("itemId")).toBe(ITEM_ID);
    expect(seen.owners[0]!.searchParams.get("sortBy")).toBe("issuedId");
    expect(seen.owners[0]!.searchParams.get("orderDirection")).toBe("asc");

    const sales = await screen.findByTestId("latest-sales");
    const rows = within(sales).getAllByTestId("sale-row");
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveTextContent("Mint");
    expect(rows[1]).toHaveTextContent("Listing");
    expect(rows[1]).toHaveTextContent("2.5");
    expect(within(rows[1]!).getAllByRole("link")[1]!.getAttribute("href")).toBe(`/shop/accounts/${BUYER}`);
    expect(seen.sales[0]!.searchParams.get("itemId")).toBe(ITEM_ID);
    expect(seen.sales[0]!.searchParams.get("tokenId")).toBeNull();
  });

  it("the token route lists that token's sales only, and no owners table", async () => {
    const seen = { sales: [] as URL[], owners: [] as URL[] };
    tokenPage({ client: catalogClient("ok", answers(seen)) });
    expect(await screen.findByTestId("latest-sales")).toBeTruthy();
    expect(seen.sales[0]!.searchParams.get("tokenId")).toBe(TOKEN_ID);
    expect(seen.sales[0]!.searchParams.get("itemId")).toBeNull();
    expect(screen.queryByTestId("item-owners")).toBeNull();
    expect(seen.owners).toHaveLength(0);
  });

  it("pages the sales five at a time", async () => {
    const seen = { sales: [] as URL[], owners: [] as URL[] };
    itemPage({ overrides: answers(seen, { sales: 7, owners: 1 }) });
    const sales = await screen.findByTestId("latest-sales");
    expect(within(sales).getAllByTestId("sale-row")).toHaveLength(5);
    expect(within(sales).getByRole("button", { name: "1" }).getAttribute("aria-current")).toBe("page");
    fireEvent.click(within(sales).getByRole("button", { name: "Next" }));
    await waitFor(() => expect(within(screen.getByTestId("latest-sales")).getAllByTestId("sale-row")).toHaveLength(2));
    expect(seen.sales.at(-1)!.searchParams.get("skip")).toBe("5");
    expect(seen.sales.at(-1)!.searchParams.get("first")).toBe("5");
  });

  it("the token route shows no sales until its token is known (LAND and the chain come from it)", async () => {
    const seen = { sales: [] as URL[], owners: [] as URL[] };
    const client = catalogClient("ok", { ...answers(seen), "/v1/nfts": { data: [], total: 0 } });
    tokenPage({ client });
    await waitFor(() => expect(client.calls.some((c) => c.url.includes("/v1/nfts"))).toBe(true));
    await act(async () => {});
    expect(screen.queryByTestId("latest-sales")).toBeNull();
    expect(seen.sales).toHaveLength(0);
  });

  it("pages the owners and goes back with Previous", async () => {
    const seen = { sales: [] as URL[], owners: [] as URL[] };
    itemPage({ overrides: answers(seen, { sales: 0, owners: 7 }) });
    const owners = await screen.findByTestId("item-owners");
    fireEvent.click(within(owners).getByRole("button", { name: "Next" }));
    await waitFor(() => expect(seen.owners.at(-1)!.searchParams.get("skip")).toBe("5"));
    fireEvent.click(within(screen.getByTestId("item-owners")).getByRole("button", { name: "Previous" }));
    await waitFor(() => expect(within(screen.getByTestId("item-owners")).getByRole("button", { name: "1" }).getAttribute("aria-current")).toBe("page"));
  });

  it("keeps the pager when a later page comes back empty, so the first page is still reachable", async () => {
    const seen = { sales: [] as URL[], owners: [] as URL[] };
    const overrides = answers(seen, { sales: 7, owners: 0 });
    const real = overrides["/v1/sales"] as (url: URL) => { data: unknown[]; total: number };
    overrides["/v1/sales"] = (url: URL) => (url.searchParams.get("skip") === "5" ? { data: [], total: 7 } : real(url));
    itemPage({ overrides });
    const sales = await screen.findByTestId("latest-sales");
    fireEvent.click(within(sales).getByRole("button", { name: "Next" }));
    await waitFor(() => expect(within(screen.getByTestId("latest-sales")).queryAllByTestId("sale-row")).toHaveLength(0));
    fireEvent.click(within(screen.getByTestId("latest-sales")).getByRole("button", { name: "Previous" }));
    await waitFor(() => expect(within(screen.getByTestId("latest-sales")).getAllByTestId("sale-row")).toHaveLength(5));
  });

  it("prices each sale in its own chain's MANA", async () => {
    const seen = { sales: [] as URL[], owners: [] as URL[] };
    itemPage({ overrides: { ...answers(seen), "/v1/sales": () => ({ data: [{ ...sale(1), chainId: 1, network: "ETHEREUM" }, sale(2)], total: 2 }) } });
    const rows = within(await screen.findByTestId("latest-sales")).getAllByTestId("sale-row");
    expect(rows[0]!.querySelector("svg")!.getAttribute("viewBox")).toBe("0 0 13 14");
    expect(rows[1]!.querySelector("svg")!.getAttribute("viewBox")).toBe("0 0 24 24");
  });

  it("says a later page failed, with a retry, instead of an empty table", async () => {
    const seen = { sales: [] as URL[], owners: [] as URL[] };
    const overrides = answers(seen, { sales: 7, owners: 0 });
    const real = overrides["/v1/sales"] as (url: URL) => unknown;
    let fail = true;
    overrides["/v1/sales"] = (url: URL) => {
      if (url.searchParams.get("skip") === "5" && fail) throw new Error("upstream down");
      return real(url);
    };
    itemPage({ overrides });
    fireEvent.click(within(await screen.findByTestId("latest-sales")).getByRole("button", { name: "Next" }));
    const error = await screen.findByTestId("history-error");
    fail = false;
    fireEvent.click(within(error).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(within(screen.getByTestId("latest-sales")).getAllByTestId("sale-row")).toHaveLength(2));
    expect(screen.queryByTestId("history-error")).toBeNull();
  });

  it("shows neither section while there are no sales and no owners", async () => {
    const seen = { sales: [] as URL[], owners: [] as URL[] };
    itemPage({ overrides: answers(seen, { sales: 0, owners: 0 }) });
    expect(await screen.findByRole("heading", { level: 1, name: "Nacho Hat" })).toBeTruthy();
    await waitFor(() => expect(seen.sales.length).toBeGreaterThan(0));
    expect(screen.queryByTestId("latest-sales")).toBeNull();
    expect(screen.queryByTestId("item-owners")).toBeNull();
  });
});
