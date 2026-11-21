// The item page reads "Suggested for you" once at the size the cart page also reads, shows fewer, and reads the
// answer with its item in the cart ahead, so Add to cart does not reload the rail behind the cart drawer.

import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { shopCart } from "../../cart/store";
import { catalogClient, catalogRow, PRIMARY_WEARABLE, ShopHarness } from "../fixtures";
import { cartInputFor } from "./AssetCard";
import { RAIL_SIZE, useSuggestedForYou } from "./SuggestedForYouRow";
import { toCatalogItem } from "@shop/discovery/lib/catalogItem";

const ON = { "dapps-shop-suggested-for-you": true };
const rows = [1, 2, 3].map((n) => ({ ...catalogRow({ ...PRIMARY_WEARABLE, itemId: String(100 + n), name: `Hat ${n}` }), reason: { kind: "seed_similar" } }));
const page = toCatalogItem(catalogRow(PRIMARY_WEARABLE));
const input = cartInputFor(page)!;
const next = { contractAddress: input.asset.contract, itemId: page.itemId! };
const pageKey = `${page.contractAddress.toLowerCase()}-${page.itemId}`;

function Probe({ nextCartItem }: { nextCartItem: typeof next | null }) {
  const s = useSuggestedForYou(RAIL_SIZE, { exclude: [pageKey], show: 2, nextCartItem });
  return <span data-testid="rows">{s.isLoading ? "loading" : String(s.result?.data.length ?? "none")}</span>;
}

function mount(nextCartItem: typeof next | null) {
  const client = catalogClient("ok", { "/v3/catalog/suggested": () => ({ data: rows, personalized: true, algorithm: "fixture" }) });
  render(
    <ShopHarness session="web3" client={client} remoteFlags={ON}>
      <Probe nextCartItem={nextCartItem} />
    </ShopHarness>,
  );
  const reads = () => client.calls.map((c) => new URL(c.url, "http://shop.test")).filter((u) => u.pathname.endsWith("/suggested"));
  return { reads };
}

const settle = () => act(async () => new Promise((r) => setTimeout(r, 50)));

// The read ahead waits for an idle moment; here every moment is idle.
beforeEach(() => {
  vi.stubGlobal("requestIdleCallback", (run: () => void) => setTimeout(run, 0));
  vi.stubGlobal("cancelIdleCallback", (handle: ReturnType<typeof setTimeout>) => clearTimeout(handle));
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  shopCart.clear();
});

describe("useSuggestedForYou on the item page", () => {
  it("reads the cart page's size and keeps the first rows it shows", async () => {
    const { reads } = mount(null);
    await settle();
    expect(reads().map((u) => u.searchParams.get("first"))).toEqual([String(RAIL_SIZE)]);
    expect(screen.getByTestId("rows").textContent).toBe("2");
  });

  it("reads the answer with the item in the cart ahead, so adding it makes no new read", async () => {
    const { reads } = mount(next);
    await waitFor(() => expect(reads().map((u) => u.searchParams.get("seeds"))).toEqual([null, pageKey]));
    await settle();
    act(() => void shopCart.add(input));
    expect(screen.getByTestId("rows").textContent).toBe("2");
    await settle();
    expect(reads()).toHaveLength(2);
  });

  it("reads nothing ahead when the item is in the cart already", async () => {
    shopCart.add(input);
    const { reads } = mount(next);
    await settle();
    expect(reads().map((u) => u.searchParams.get("seeds"))).toEqual([pageKey]);
  });
});
