
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ReactElement } from "react";
import { useLocation } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { catalogClient, COLLECTION, CREATOR, ShopHarness, type CatalogAnswers, type CatalogScenario } from "../fixtures";
import { invalidateShopQueries } from "../query";
import Collection from "./Collection";
import Creator from "./Creator";
import Event from "./Event";
import NotFound from "./NotFound";

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response("{}", { status: 200 })));
  invalidateShopQueries();
});
afterEach(() => vi.unstubAllGlobals());

function Where() {
  const { pathname, search } = useLocation();
  return <output data-testid="where">{pathname + search}</output>;
}

function mount(path: string, entry: string, element: ReactElement, scenario: CatalogScenario = "ok", overrides?: CatalogAnswers) {
  const client = catalogClient(scenario, overrides);
  render(
    <ShopHarness path={path} entry={entry} client={client}>
      <>
        {element}
        <Where />
      </>
    </ShopHarness>,
  );
  return client;
}

describe("the collection storefront", () => {
  it("names the collection and lists every item of it", async () => {
    const client = mount("/shop/collection/:contractAddress", `/shop/collection/${COLLECTION}`, <Collection />);
    expect(await screen.findByTestId("collection-hero-title")).toHaveTextContent("Nacho Collection");
    expect(await screen.findAllByTestId("card")).toHaveLength(3);
    const items = client.calls.map((c) => new URL(c.url)).find((u) => u.pathname === "/shop-api/v3/catalog/items")!;
    expect(items.searchParams.get("contractAddress")).toBe(COLLECTION);
    expect(items.searchParams.get("isOnSale")).toBeNull();
  });

  it("empty: says the collection has nothing, and offers to clear filters", async () => {
    mount("/shop/collection/:contractAddress", `/shop/collection/${COLLECTION}`, <Collection />, "empty");
    expect(await screen.findByTestId("collection-empty")).toHaveTextContent("This collection has no items to show.");
  });

  it("error: says so with a retry", async () => {
    mount("/shop/collection/:contractAddress", `/shop/collection/${COLLECTION}`, <Collection />, "error");
    expect(await screen.findByTestId("collection-error")).toHaveTextContent("Couldn't load this collection");
  });

  it("an index outage is explained", async () => {
    mount("/shop/collection/:contractAddress", `/shop/collection/${COLLECTION}`, <Collection />, "outage");
    expect(await screen.findByTestId("index-notice")).toBeTruthy();
    expect(screen.queryByTestId("collection-empty")).toBeNull();
  });
});

describe("the creator storefront", () => {
  it("lists the creator's items and switches to their collections", async () => {
    mount("/shop/items/creator/:address", `/shop/items/creator/${CREATOR}`, <Creator />);
    expect(await screen.findAllByTestId("card")).toHaveLength(3);
    fireEvent.click(await screen.findByTestId("creator-filter-collections"));
    await waitFor(() => expect(screen.getByTestId("where")).toHaveTextContent("collections"));
    expect(await screen.findByTestId("collection-card")).toHaveTextContent("Nacho Collection");
  });

  it("tells a creator with no work from a filter that excludes everything", async () => {
    mount("/shop/items/creator/:address", `/shop/items/creator/${CREATOR}`, <Creator />, "empty");
    expect(await screen.findByTestId("creator-empty")).toHaveTextContent("This creator hasn't published anything yet.");
  });

  it("filters that exclude everything say how many items the creator has", async () => {
    mount("/shop/items/creator/:address", `/shop/items/creator/${CREATOR}?rarities=unique`, <Creator />, "ok", {
      "/v3/catalog/items": (url: URL) => (url.searchParams.get("rarity") ? { data: [], total: 0 } : { data: [], total: 7 }),
    });
    expect(await screen.findByTestId("creator-empty-filters")).toHaveTextContent("This creator has 7 items");
  });

  it("error: says so with a retry", async () => {
    mount("/shop/items/creator/:address", `/shop/items/creator/${CREATOR}`, <Creator />, "error");
    expect(await screen.findByTestId("creator-error")).toHaveTextContent("Couldn't load this creator");
  });
});

describe("the event page", () => {
  it("with no campaign configured, goes to the ordinary grid", async () => {
    mount("/shop/*", "/shop/event", <Event />);
    await waitFor(() => expect(screen.getByTestId("where")).toHaveTextContent("/shop/items"));
  });
});

describe("not found", () => {
  it("offers a way back into the Shop", () => {
    mount("/shop/*", "/shop/nope", <NotFound />);
    const page = screen.getByTestId("notfound");
    expect(page).toHaveTextContent("Page not found");
    expect(within(page).getByText("Browse Collectibles").getAttribute("href")).toBe("/shop/items");
  });
});
