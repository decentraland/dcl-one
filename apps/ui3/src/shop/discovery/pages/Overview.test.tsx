// The home page's states: skeleton rails while loading, the rails and their cards priced under the
// payment rule, the empty state, a failed read with a retry, and an index without a complete pass shown
// as catching up rather than as "no drops".

import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { PRIMARY_WEARABLE, RESALE_WEARABLE, ShopHarness, type CatalogAnswers, type CatalogScenario, type SessionKind } from "../fixtures";
import { invalidateShopQueries } from "../query";
import Overview from "./Overview";

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response("{}", { status: 200 })));
  invalidateShopQueries();
});
afterEach(() => vi.unstubAllGlobals());

function overview(scenario: CatalogScenario = "ok", session: SessionKind = "signedOut", overrides?: CatalogAnswers) {
  return render(
    <ShopHarness path="/shop/overview" entry="/shop/overview" scenario={scenario} session={session} overrides={overrides}>
      <Overview />
    </ShopHarness>,
  );
}

/** Lets the reads that resolve without a visible change (the hero's profile) land inside act. */
const settle = () => act(async () => {});

describe("the Shop overview", () => {
  it("loading: the rails hold skeleton cards", async () => {
    overview("loading");
    await settle();
    expect(screen.getByTestId("hero-title")).toHaveTextContent("A New Way to Shop");
    expect(within(screen.getByTestId("new-creations-rail")).getAllByTestId("skeleton-card").length).toBeGreaterThan(0);
    expect(within(screen.getByTestId("trending-rail")).getAllByTestId("skeleton-card").length).toBeGreaterThan(0);
  });

  it("renders the rails with credits-first cards, the deals rail only with enough deals", async () => {
    overview();
    const creations = await screen.findByTestId("new-creations-rail");
    const cards = await within(creations).findAllByTestId("card");
    expect(cards.length).toBeGreaterThan(0);
    const hat = cards.find((c) => within(c).queryByText("Nacho Hat") && !within(c).queryByTestId("card-resale"))!;
    expect(within(hat).getByTestId("card-price")).toHaveTextContent("12");
    expect(screen.queryByTestId("best-deals-rail")).toBeNull();
    expect(screen.getByTestId("hero-credits-cta").getAttribute("href")).toBe("/shop/credits");
  });

  it("New Creations asks for primaries only; a resale in Trending is MANA only, never its credit figure", async () => {
    overview("ok", "web3", { "/v3/catalog/trending": { data: [PRIMARY_WEARABLE, RESALE_WEARABLE] } });
    const creations = await screen.findByTestId("new-creations-rail");
    expect(within(creations).queryByTestId("card-resale")).toBeNull();
    const rail = await screen.findByTestId("trending-rail");
    const resale = (await within(rail).findAllByTestId("card")).find((c) => within(c).queryByTestId("card-resale"));
    expect(resale).toBeDefined();
    expect(within(resale!).getByTestId("card-price")).toHaveTextContent("MANA");
    expect(resale!.textContent).not.toMatch(/Credit/);
  });

  it("hides the credits CTA in the iOS web view", async () => {
    window.history.pushState({}, "", "/?view=mobile-iap");
    overview();
    await within(await screen.findByTestId("new-creations-rail")).findAllByTestId("card");
    expect(screen.queryByTestId("hero-credits-cta")).toBeNull();
    window.history.pushState({}, "", "/");
  });

  it("empty: new drops are on the way", async () => {
    overview("empty");
    expect(await screen.findByTestId("overview-empty")).toHaveTextContent("New drops are on the way");
    expect(screen.queryByTestId("trending-rail")).toBeNull();
  });

  it("a failed read says so, and retries", async () => {
    overview("error");
    const error = await screen.findByTestId("overview-error");
    expect(error).toHaveTextContent("could not be loaded");
    act(() => invalidateShopQueries());
    fireEvent.click(within(error).getByText("Try again"));
    await waitFor(() => expect(screen.getByTestId("overview-error")).toBeTruthy());
  });

  it("an index outage is explained, not shown as an empty shop", async () => {
    overview("outage");
    const notice = await screen.findByTestId("index-notice");
    expect(notice).toHaveTextContent("New Creations is unavailable right now: the items mirror has no complete pass yet");
    expect(screen.queryByTestId("overview-empty")).toBeNull();
  });

  it("web3: a MANA-priced mint shows credits first and its exact MANA second; web2 sees credits only", async () => {
    overview("ok", "web3");
    const rail = await screen.findByTestId("trending-rail");
    const sax = (await within(rail).findAllByTestId("card")).find((c) => within(c).queryByText("Hot Sax"))!;
    expect(within(sax).getByTestId("card-price")).toHaveTextContent("21");
    expect(within(sax).getByTestId("card-price-mana")).toHaveTextContent("or 4.02 MANA");
  });

  it("web2: no MANA on an eligible card", async () => {
    overview("ok", "web2");
    const rail = await screen.findByTestId("trending-rail");
    const sax = (await within(rail).findAllByTestId("card")).find((c) => within(c).queryByText("Hot Sax"))!;
    expect(within(sax).queryByTestId("card-price-mana")).toBeNull();
  });
});
