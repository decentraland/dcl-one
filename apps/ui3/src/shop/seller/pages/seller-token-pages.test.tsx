import { act, fireEvent, screen } from "@testing-library/react";
import { decodeFunctionData, type Hex } from "viem";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { CHAIN } from "../../model/chain";
import { BOOK } from "../../payment/fixtures";
import { MANA, nftRow } from "../../market/fixtures";
import { COLLECTION_V2_ABI, ERC20_ABI, ERC721_ABI, latestOffChainMarketplace, MARKETPLACE_ABI, manaToken } from "../../market/lib/contracts";
import { createManaListing } from "../../market/lib/trades";
import { fakeChain, fakeWallet, type FakeRoute } from "../../market/testing";
import { mountShop } from "../../market/testHarness";
import { setShopCapabilities } from "../lib/featureFlags";

const MARKET = latestOffChainMarketplace(CHAIN.POLYGON).address;
let restoreChain: (() => void) | null = null;
const originalFetch = globalThis.fetch;
let relayOk = true;
let relayed: Hex[] = [];

beforeEach(() => {
  relayOk = true;
  relayed = [];
  globalThis.fetch = (async (url: string, init?: RequestInit) => {
    if (String(url).endsWith("/transactions")) {
      relayed.push((JSON.parse(String(init?.body)) as { transactionData: { params: [string, Hex] } }).transactionData.params[1]);
      return relayOk ? new Response(JSON.stringify({ ok: true, txHash: `0x${"e".repeat(64)}` })) : new Response(JSON.stringify({ ok: false, message: "relay down", code: "UNKNOWN" }), { status: 503 });
    }
    throw new Error(`unexpected fetch ${String(url)}`);
  }) as typeof fetch;
  restoreChain = fakeChain({
    contracts: {
      [MARKET]: { abi: MARKETPLACE_ABI, answer: () => 0n },
      [BOOK.collection]: { abi: [...ERC721_ABI, ...COLLECTION_V2_ABI], answer: (fn) => (fn === "isApprovedForAll" ? true : 0n) },
      [manaToken(CHAIN.POLYGON).address]: { abi: [...ERC20_ABI, ...COLLECTION_V2_ABI], answer: () => 0n },
    },
  });
});

afterEach(() => {
  restoreChain?.();
  globalThis.fetch = originalFetch;
  act(() => setShopCapabilities(null));
});

const nfts = (row: ReturnType<typeof nftRow>): FakeRoute => ({ method: "GET", path: /^\/v1\/nfts$/, reply: () => ({ body: { data: [row], total: 1 } }) });

describe("List a token for sale: a resale, MANA only", () => {
  it("signs a MANA public_nft_order for the owner and never sets or shows a credit price", async () => {
    const wallet = fakeWallet();
    const posted: unknown[] = [];
    mountShop({
      path: `/shop/token/${BOOK.collection}/105313/sell`,
      who: "web3",
      wallet,
      routes: [
        nfts(nftRow({ tokenId: "105313", itemId: "8", name: "Cyber Boots", owner: wallet.address })),
        { method: "POST", path: /^\/v1\/trades$/, reply: ({ body }) => (posted.push(body), { status: 201, body: { ok: true, data: { ...(body as object), id: "list-1", contract: MARKET } } }) },
      ],
    });
    expect(await screen.findByText(/Buyers cannot pay for a resale with Credits/)).toBeInTheDocument();
    fireEvent.change(screen.getByPlaceholderText("1000"), { target: { value: "7.25" } });
    fireEvent.click(screen.getByRole("button", { name: "List for sale" }));
    const proceed = await screen.findByRole("button", { name: "Proceed" });
    expect(proceed).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Confirm the price"), { target: { value: "7.25" } });
    fireEvent.click(proceed);
    expect(await screen.findByTestId("shop-sell-done")).toHaveTextContent("7.25 MANA");
    expect(posted[0]).toMatchObject({ type: "public_nft_order", sent: [{ assetType: 3, tokenId: "105313" }], received: [{ assetType: 1, amount: MANA("7.25"), beneficiary: wallet.address }] });
  });

  it("repricing takes the old signed listing down first", async () => {
    const wallet = fakeWallet();
    const old = await createManaListing({ provider: wallet, nft: { contractAddress: BOOK.collection, tokenId: "105312", chainId: CHAIN.POLYGON }, priceWei: 1n, expiresAtMs: Date.now() + 1000 });
    mountShop({
      path: `/shop/token/${BOOK.collection}/105312/sell`,
      who: "web3",
      wallet,
      routes: [
        nfts(nftRow({ tokenId: "105312", itemId: "7", name: "Neon Jacket", owner: wallet.address, onSale: { price: MANA(1), tradeId: "old-1" } })),
        { method: "GET", path: /^\/v1\/trades\/old-1$/, reply: () => ({ body: { ok: true, data: { ...old, id: "old-1", contract: MARKET } } }) },
        { method: "POST", path: /^\/v1\/trades$/, reply: ({ body }) => ({ status: 201, body: { ok: true, data: { ...(body as object), id: "list-2", contract: MARKET } } }) },
      ],
    });
    fireEvent.change(await screen.findByPlaceholderText("1000"), { target: { value: "3" } });
    fireEvent.click(screen.getByRole("button", { name: "Update" }));
    fireEvent.change(await screen.findByLabelText("Confirm the price"), { target: { value: "3" } });
    fireEvent.click(screen.getByRole("button", { name: "Proceed" }));
    expect(await screen.findByTestId("shop-sell-done")).toBeInTheDocument();
    expect(decodeFunctionData({ abi: MARKETPLACE_ABI, data: decodeFunctionData({ abi: MARKETPLACE_ABI, data: relayed[0]! }).args![1] as Hex }).functionName).toBe("cancelSignature");
  });

  it("strangers and web2 sessions get no form; a node without resales says so", async () => {
    const wallet = fakeWallet();
    const view = mountShop({ path: `/shop/token/${BOOK.collection}/105313/sell`, who: "web3", wallet, routes: [nfts(nftRow({ tokenId: "105313", name: "x", owner: BOOK.reseller }))] });
    expect(await screen.findByTestId("shop-not-owner")).toBeInTheDocument();
    view.unmount();
    const web2 = mountShop({ path: `/shop/token/${BOOK.collection}/105313/sell`, who: "web2", wallet, routes: [nfts(nftRow({ tokenId: "105313", name: "x", owner: wallet.address }))] });
    expect(await screen.findByTestId("shop-wallet-required")).toBeInTheDocument();
    web2.unmount();
    setShopCapabilities({ secondarySales: false });
    mountShop({ path: `/shop/token/${BOOK.collection}/105313/sell`, who: "web3", wallet, routes: [nfts(nftRow({ tokenId: "105313", name: "x", owner: wallet.address }))] });
    expect(await screen.findByTestId("shop-resales-off")).toBeInTheDocument();
  });
});

describe("Remove from sale", () => {
  it("asks before paying gas when the free cancellation fails, then cancels from that click", async () => {
    const wallet = fakeWallet();
    const old = await createManaListing({ provider: wallet, nft: { contractAddress: BOOK.collection, tokenId: "105312", chainId: CHAIN.POLYGON }, priceWei: 1n, expiresAtMs: Date.now() + 1000 });
    relayOk = false;
    mountShop({
      path: `/shop/token/${BOOK.collection}/105312/cancel`,
      who: "web3",
      wallet,
      routes: [
        nfts(nftRow({ tokenId: "105312", itemId: "7", name: "Neon Jacket", owner: wallet.address, onSale: { price: MANA(1), tradeId: "old-1" } })),
        { method: "GET", path: /^\/v1\/trades\/old-1$/, reply: () => ({ body: { ok: true, data: { ...old, id: "old-1", contract: MARKET } } }) },
      ],
    });
    fireEvent.click(await screen.findByRole("button", { name: "Confirm" }));
    expect(await screen.findByTestId("shop-gasless-failed")).toBeInTheDocument();
    expect(wallet.sent).toHaveLength(0);
    fireEvent.click(screen.getByRole("button", { name: "Cancel and pay the gas" }));
    expect(await screen.findByTestId("shop-remove-done")).toBeInTheDocument();
    expect(decodeFunctionData({ abi: MARKETPLACE_ABI, data: wallet.sent[0]!.data }).functionName).toBe("cancelSignature");
  });

  it("a token that is not on sale says so, and Confirm stays disabled", async () => {
    const wallet = fakeWallet();
    mountShop({ path: `/shop/token/${BOOK.collection}/105313/cancel`, who: "web3", wallet, routes: [nfts(nftRow({ tokenId: "105313", name: "x", owner: wallet.address }))] });
    expect(await screen.findByTestId("shop-not-on-sale")).toHaveTextContent("There are no active listings for x.");
    expect(screen.getByRole("button", { name: "Confirm" })).toBeDisabled();
  });

  it("signed out: the Marketplace's sign-in page, on both sell and cancel", async () => {
    const view = mountShop({ path: `/shop/token/${BOOK.collection}/105313/cancel`, who: "signedOut" });
    expect(await screen.findByTestId("shop-signin-gate")).toHaveTextContent("Get Started");
    view.unmount();
    mountShop({ path: `/shop/token/${BOOK.collection}/105313/sell`, who: "signedOut" });
    expect(await screen.findByTestId("shop-signin-gate")).toHaveTextContent("Get Started");
  });
});
