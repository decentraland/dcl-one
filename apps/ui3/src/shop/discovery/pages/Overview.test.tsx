// The home page's states: skeleton rails while loading, the rails and their cards priced under the
// payment rule, the empty state, a failed read with a retry, and an index without a complete pass shown
// as catching up rather than as "no drops".

import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ReactNode } from "react";

import { fetchShopItems } from "@shop/discovery/api";
import { setShopClient } from "../../api/host";
import { catalogClient, PRIMARY_WEARABLE, RESALE_WEARABLE, saleWearable, ShopHarness, type CatalogAnswers, type CatalogScenario, type SessionKind } from "../fixtures";
import { primeRemoteFlags } from "../lib/remoteFlags";
import { invalidateShopQueries } from "../query";
import { shopReadsAfter } from "../reads";
import Overview from "./Overview";
import { prefetchOverviewRails } from "./overviewRails";

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response("{}", { status: 200 })));
  invalidateShopQueries();
});
afterEach(() => vi.unstubAllGlobals());

function overview(scenario: CatalogScenario = "ok", session: SessionKind = "signedOut", overrides?: CatalogAnswers, remoteFlags?: Record<string, boolean>) {
  return render(
    <ShopHarness path="/shop/overview" entry="/shop/overview" scenario={scenario} session={session} overrides={overrides} remoteFlags={remoteFlags}>
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
    expect(within(screen.getByRole("region", { name: "New Creations" })).getAllByTestId("skeleton-card").length).toBeGreaterThan(0);
    expect(within(screen.getByTestId("trending-rail")).getAllByTestId("skeleton-card").length).toBeGreaterThan(0);
  });

  it("renders the rails with credits-first cards, the deals rail only with enough deals", async () => {
    overview();
    const creations = await screen.findByRole("region", { name: "New Creations" });
    const cards = await within(creations).findAllByTestId("card");
    expect(cards.length).toBeGreaterThan(0);
    const hat = cards.find((c) => within(c).queryByText("Nacho Hat") && !within(c).queryByTestId("card-resale"))!;
    expect(within(hat).getByTestId("card-price")).toHaveTextContent("12");
    expect(screen.queryByTestId("best-deals-rail")).toBeNull();
    expect(screen.getByTestId("hero-credits-cta").getAttribute("href")).toBe("/shop/credits");
  });

  it("New Creations asks for primaries only; a resale in Trending is MANA only, never its credit figure", async () => {
    overview("ok", "web3", { "/v3/catalog/trending": { data: [PRIMARY_WEARABLE, RESALE_WEARABLE] } });
    const creations = await screen.findByRole("region", { name: "New Creations" });
    expect(within(creations).queryByTestId("card-resale")).toBeNull();
    const rail = await screen.findByTestId("trending-rail");
    const resale = (await within(rail).findAllByTestId("card")).find((c) => within(c).queryByTestId("card-resale"));
    expect(resale).toBeDefined();
    expect(within(resale!).getByTestId("card-price")).toHaveTextContent("MANA");
    expect(resale!.textContent).not.toMatch(/Credit/);
  });

  it("shows Best Deals once decentraland.org's creator-sales rollout is on, and not before", async () => {
    const deals = [1, 2, 3].map((n) => ({ ...saleWearable(), tradeId: `trade-sale-${n}`, itemId: `${n}` }));
    const asked: string[] = [];
    const answers: CatalogAnswers = {
      "/v3/catalog/unified": (url: URL) => {
        asked.push(url.search);
        return url.searchParams.get("discounted") === "true" ? { data: deals, total: deals.length } : { data: [PRIMARY_WEARABLE], total: 1 };
      },
    };
    const off = overview("ok", "signedOut", answers, {});
    await screen.findByRole("region", { name: "New Creations" });
    expect(screen.queryByTestId("best-deals-rail")).toBeNull();
    expect(asked.some((q) => q.includes("discounted=true"))).toBe(false);
    off.unmount();

    invalidateShopQueries();
    overview("ok", "signedOut", answers, { "dapps-shop-creator-sales": true });
    expect(await screen.findByTestId("best-deals-rail")).toBeTruthy();
    expect(asked.some((q) => q.includes("discounted=true"))).toBe(true);
  });

  it("hides the credits CTA in the iOS web view", async () => {
    window.history.pushState({}, "", "/?view=mobile-iap");
    overview();
    await within(await screen.findByRole("region", { name: "New Creations" })).findAllByTestId("card");
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

  it("a link's loader sends the rails before the page mounts; the page draws them without asking again", async () => {
    const client = catalogClient();
    const page = (children: ReactNode) => (
      <ShopHarness path="/shop/overview" entry="/shop/overview" client={client} remoteFlags={{ "dapps-shop-creator-sales": true }}>
        {children}
      </ShopHarness>
    );
    const view = render(page(null));
    prefetchOverviewRails(Promise.resolve());
    const asked = (part: string) => client.calls.filter((c) => c.url.includes(part)).length;
    const rails = () => [asked("sortBy=newest"), asked("/catalog/trending"), asked("discounted=true")];
    expect(rails()).toEqual([1, 1, 1]);
    view.rerender(page(<Overview />));
    await within(await screen.findByRole("region", { name: "New Creations" })).findAllByTestId("card");
    await within(screen.getByTestId("trending-rail")).findAllByTestId("card");
    expect(rails()).toEqual([1, 1, 1]);
  });

  it("an answer sent before the flags are known is mapped with their answer", async () => {
    setShopClient(catalogClient());
    primeRemoteFlags(null);
    let answer!: () => void;
    const flags = new Promise<void>((resolve) => (answer = resolve));
    let landed = false;
    const read = fetchShopItems({ first: 12, discounted: true, sortBy: "discount", listingType: "primary" }, shopReadsAfter(flags)(new AbortController().signal));
    void read.then(() => (landed = true));
    await settle();
    expect(landed).toBe(false);
    primeRemoteFlags({ "dapps-shop-creator-sales": true });
    answer();
    expect((await read).items[0]?.compareAtCredits).toBeGreaterThan(0);
    setShopClient(null);
  });

  it("sends Best Deals only once unknown flags say creator sales are on", async () => {
    for (const [creatorSales, deals] of [
      [false, 0],
      [true, 1],
    ] as const) {
      invalidateShopQueries();
      const client = catalogClient();
      setShopClient(client);
      primeRemoteFlags(null);
      let answer!: () => void;
      const flags = new Promise<void>((resolve) => (answer = resolve));
      prefetchOverviewRails(flags);
      const asked = (part: string) => client.calls.filter((c) => c.url.includes(part)).length;
      expect([asked("sortBy=newest"), asked("/catalog/trending"), asked("discounted=true")]).toEqual([1, 1, 0]);
      primeRemoteFlags({ "dapps-shop-creator-sales": creatorSales });
      answer();
      await settle();
      expect(asked("discounted=true"), `creator sales ${creatorSales}`).toBe(deals);
    }
    setShopClient(null);
  });
});
