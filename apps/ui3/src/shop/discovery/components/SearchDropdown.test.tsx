// The search box: recent and popular searches while empty, the suggestion sections and a facet while
// typing, keyboard navigation over one flat list, Enter running the search on the grid, and the clear
// button dropping only the query.

import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useLocation } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { catalogClient, ShopHarness } from "../fixtures";
import { clearRecentSearches, recordSearch } from "../lib/recent-searches";
import SearchDropdown from "./SearchDropdown";

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response("{}", { status: 200 })));
  clearRecentSearches();
});
afterEach(() => vi.unstubAllGlobals());

function Where() {
  const { pathname, search } = useLocation();
  return <output data-testid="where">{pathname + search}</output>;
}

function box(entry = "/shop/overview", scenario: "ok" | "error" = "ok") {
  const client = catalogClient(scenario);
  render(
    <ShopHarness path="/shop/*" entry={entry} client={client}>
      <>
        <SearchDropdown debounceMs={0} />
        <Where />
      </>
    </ShopHarness>,
  );
  return { client, input: screen.getByTestId("search-input") as HTMLInputElement };
}

describe("the search panel as decentraland.org's Shop draws it", () => {
  it("offers popular searches as chips, and keeps each recent search's removal out of its option", () => {
    recordSearch("pirate");
    const { input } = box();
    fireEvent.focus(input);
    const panel = screen.getByTestId("search-panel");
    const chips = panel.querySelectorAll(".shop-search__chip");
    expect(chips.length).toBeGreaterThan(3);
    expect([...chips].every((c) => c.getAttribute("role") === "option")).toBe(true);
    const recent = within(panel).getAllByRole("option")[0]!;
    expect(recent.textContent).toBe("pirate");
    expect(within(recent).queryByRole("button")).toBeNull();
    expect(within(panel).getByRole("button", { name: "Remove “pirate” from recent searches" })).toBeTruthy();
    expect(within(panel).queryByText("Explore Shop")).toBeNull();
  });

  it("names each item's and collection's creator, marks the match in bold, and draws the collection as a mosaic", async () => {
    const { input } = box();
    fireEvent.change(input, { target: { value: "nacho" } });
    const panel = await screen.findByTestId("search-panel");
    await waitFor(() => expect(within(panel).getAllByText("By Nacho").length).toBeGreaterThanOrEqual(2));
    const item = within(panel).getAllByRole("option").find((o) => o.getAttribute("data-kind") === "item")!;
    expect(item.querySelector("mark")?.textContent).toBe("Nacho");
    expect(item.querySelector(".shop-search__thumb img")).toBeTruthy();
    const collection = within(panel).getAllByRole("option").find((o) => o.getAttribute("data-kind") === "collection")!;
    await waitFor(() => expect(collection.querySelector(".shop-search__coll [data-testid='coll-thumb-cell']")).toBeTruthy());
  });
});

describe("the search box", () => {
  it("while empty offers recent searches first, then popular ones not already recent", () => {
    recordSearch("duck");
    recordSearch("pirate");
    const { input } = box();
    fireEvent.focus(input);
    const panel = screen.getByTestId("search-panel");
    const options = within(panel).getAllByRole("option").map((o) => o.textContent);
    expect(options.slice(0, 2)).toEqual(["pirate", "duck"]);
    expect(options).toContain("sword");
    expect(options.filter((o) => o === "duck")).toHaveLength(1);
  });

  it("typing shows the facet, items, collections and creators, and See all", async () => {
    const { input, client } = box();
    fireEvent.change(input, { target: { value: "hat" } });
    const panel = await screen.findByTestId("search-panel");
    await waitFor(() => expect(within(panel).getAllByRole("option").length).toBeGreaterThan(3));
    const texts = within(panel).getAllByRole("option").map((o) => o.textContent ?? "");
    expect(texts[0]).toContain("Hat");
    expect(texts.some((t) => t.includes("Nacho Hat"))).toBe(true);
    expect(texts.some((t) => t.includes("Nacho Collection"))).toBe(true);
    expect(texts.at(-1)).toBe("See all 1 results");
    // One suggest request answers the three sections (the collection row's mosaic then reads its own items).
    const suggests = client.calls.map((c) => new URL(c.url)).filter((u) => u.pathname === "/shop-api/v3/catalog/suggest");
    expect(suggests).toHaveLength(1);
    expect(suggests[0]!.searchParams.get("search")).toBe("hat");
  });

  it("arrows move through one flat list and Enter activates the active row", async () => {
    const { input } = box();
    fireEvent.change(input, { target: { value: "nacho" } });
    await waitFor(() => expect(screen.getAllByRole("option").length).toBeGreaterThan(2));
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(input.getAttribute("aria-activedescendant")).toBe(screen.getAllByRole("option")[0]!.id);
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(screen.getByTestId("where")).toHaveTextContent("/shop/item/"));
  });

  it("Enter with no active row runs the search on the grid and remembers it", async () => {
    const { input } = box();
    fireEvent.change(input, { target: { value: "sword" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(screen.getByTestId("where")).toHaveTextContent("/shop/items?q=sword"));
    fireEvent.focus(input);
  });

  it("clearing on the results page drops the query and keeps the other filters", async () => {
    box("/shop/items?q=hat&status=not_for_sale");
    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
    await waitFor(() => expect(screen.getByTestId("where")).toHaveTextContent("/shop/items?status=not_for_sale"));
  });

  it("a failed suggestion read says so and offers a retry", async () => {
    const { input } = box("/shop/overview", "error");
    fireEvent.change(input, { target: { value: "hat" } });
    expect(await screen.findByRole("alert")).toHaveTextContent("We couldn't load suggestions.");
  });
});
