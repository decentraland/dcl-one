import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { BOOK } from "@shop/payment/fixtures";
import type { EvmAddress } from "@shop/model/address";
import { catalogClient, fixtureHost, RESELLER, ShopHarness } from "../../discovery/fixtures";
import { invalidateShopQueries } from "../../discovery/query";
import { APPROVAL_STILL_PENDING, TxPendingError } from "../../market/lib/tx";
import type { Trade } from "../../market/lib/trades";
import { buyTradeWithMana, executeLegacyOrder } from "../../market/purchase/executeOrder";
import { fakeWallet } from "../../market/testing";
import LandBranch from "./LandBranch";

// The wallet's side of the trade is not under test here: each test says how it ends.
vi.mock("../../market/purchase/executeOrder", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../market/purchase/executeOrder")>()),
  buyTradeWithMana: vi.fn(async () => `0x${"e".repeat(64)}`),
  executeLegacyOrder: vi.fn(async () => `0x${"e".repeat(64)}`),
}));

const { PRICE } = vi.hoisted(() => ({ PRICE: "4000000000000000000000" }));
vi.mock("../../market/lib/trades", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../market/lib/trades")>();
  // The listing the dialog reads: the order's own trade, priced in MANA on Ethereum.
  const trade = (id: string) => ({ id, contract: `0x${"4".repeat(40)}`, chainId: 1, sent: [], received: [{ assetType: actual.TRADE_ASSET.ERC20, amount: PRICE }] }) as unknown as Trade;
  return { ...actual, fetchTrade: vi.fn(async (id: string) => trade(id)) };
});

const BOUGHT = "77";
const NEIGHBOUR = "78";
const SALE = `0x${"d".repeat(64)}`;

type Listing = "trade" | "legacy order";
let listing: Listing = "trade";

/** A parcel listed by RESELLER, through a signed trade or a legacy on-chain order. */
const parcel = (tokenId: string) => ({
  nft: { id: `${BOOK.land}-${tokenId}`, activeOrderId: `order-${tokenId}`, category: "parcel", chainId: 1, contractAddress: BOOK.land, createdAt: 0, data: { parcel: { x: tokenId, y: "-4", description: null, estate: null } }, image: "", issuedId: null, itemId: null, name: `Parcel ${tokenId},-4`, network: "ETHEREUM", openRentalId: null, owner: RESELLER, tokenId, soldAt: 0, updatedAt: 0, url: "" },
  order: { id: `order-${tokenId}`, marketplaceAddress: `0x${"8".repeat(40)}`, contractAddress: BOOK.land, tokenId, owner: RESELLER, buyer: null, price: PRICE, status: "open", expiresAt: 1_900_000_000_000, createdAt: 1_780_000_000_000, updatedAt: 1_780_000_000_000, network: "ETHEREUM", chainId: 1, issuedId: null, tradeId: listing === "trade" ? `trade-${tokenId}` : null },
  rental: null,
});

/** Every read of one LAND (its token and open order, `/v1/nfts?tokenId=`), counted per parcel. */
let reads: Record<string, number> = {};

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response("[]", { status: 200, headers: { "content-type": "application/json" } })));
  invalidateShopQueries();
  reads = {};
});

afterEach(() => {
  vi.unstubAllGlobals();
});

/** The bought parcel's page and its neighbour's, side by side; the buyer opens Buy on the first. */
async function openBuy() {
  const wallet = fakeWallet({ chainId: 1 });
  const nfts = (url: URL) => {
    const tokenId = url.searchParams.get("tokenId");
    if (!tokenId) return { data: [], total: 0 };
    reads[tokenId] = (reads[tokenId] ?? 0) + 1;
    return { data: [parcel(tokenId)], total: 1 };
  };
  const client = catalogClient("ok", { "/v1/nfts": nfts, "/v1/items": { data: [], total: 0 }, "/v3/catalog/unified": { data: [], total: 0 } });
  const host = fixtureHost("web3", wallet.address as EvmAddress, { provider: () => wallet });
  render(
    <ShopHarness session="web3" host={host} client={client}>
      {/* The detail page's query client, which the dialog's balances live in. */}
      <QueryClientProvider client={new QueryClient()}>
        <div data-testid="bought">
          <LandBranch contractAddress={BOOK.land} tokenId={BOUGHT} viewer={wallet.address} />
        </div>
        <div data-testid="neighbour">
          <LandBranch contractAddress={BOOK.land} tokenId={NEIGHBOUR} viewer={wallet.address} />
        </div>
      </QueryClientProvider>
    </ShopHarness>,
  );
  fireEvent.click(await within(screen.getByTestId("bought")).findByTestId("land-buy"));
  const confirm = await screen.findByTestId("order-buy");
  await waitFor(() => expect(confirm).toBeEnabled());
  const before = { ...reads };
  expect(before).toEqual({ [BOUGHT]: 1, [NEIGHBOUR]: 1 });
  await act(async () => fireEvent.click(confirm));
  return before;
}

/** The dialog's own close (its corner button), as the buyer dismisses the notice. */
const close = () => fireEvent.click(screen.getAllByRole("button", { name: "Close" })[0]!);

/** How the wallet's side of the purchase ends, for the listing under test. */
function purchaseEnds(how: (opts: { onSubmitted?: (hash: string) => void }) => Promise<string>) {
  if (listing === "trade") vi.mocked(buyTradeWithMana).mockImplementationOnce(how);
  else vi.mocked(executeLegacyOrder).mockImplementationOnce(how);
}

describe.each<Listing>(["trade", "legacy order"])("a LAND bought from its page through a %s, with the sale still confirming", (kind) => {
  beforeEach(() => {
    listing = kind;
  });

  it("keeps the notice up, then reads that LAND again once it closes, and not its neighbour", async () => {
    purchaseEnds(async (opts) => {
      opts.onSubmitted?.(SALE);
      throw new TxPendingError(SALE, "The purchase");
    });
    const before = await openBuy();
    expect(await screen.findByText(/The purchase was sent and is still confirming/)).toBeTruthy();
    // Not read under the notice: a read empties the page until it answers, and would take the dialog with it.
    expect(reads).toEqual(before);

    close();
    await waitFor(() => expect(reads[BOUGHT]).toBe(before[BOUGHT]! + 1));
    expect(reads[NEIGHBOUR]).toBe(before[NEIGHBOUR]);
    expect(await within(screen.getByTestId("bought")).findByTestId("land-buy")).toBeTruthy();
  });

  it("reads nothing again when only the MANA approval was still confirming", async () => {
    purchaseEnds(async () => {
      throw new TxPendingError(`0x${"a".repeat(64)}`, "The MANA approval");
    });
    const before = await openBuy();
    expect(await screen.findByText(APPROVAL_STILL_PENDING)).toBeTruthy();

    close();
    await waitFor(() => expect(screen.queryByTestId("order-buy")).toBeNull());
    await new Promise((settled) => setTimeout(settled, 50));
    expect(reads).toEqual(before);
  });
});
