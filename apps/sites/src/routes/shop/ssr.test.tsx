import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { ComponentType } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { describe, expect, it } from "vitest";

import { createShopClient, hostApiOptions } from "@ui/shop/api";
import { SHOP_PAGES } from "@ui/shop/routes";

import { defaultShopSeo, type ShopPageData } from "@/data/lib/shop/pages";
import { loadShopPage } from "@/data/lib/shop/pages.server";

import { useMyStoreAccess } from "@dcl/one-ui/apps/shop/capabilities";
import { REMOTE_FLAG, remoteFlagVariant, useRemoteFlag } from "@dcl/one-ui/apps/shop/remote-flags";

import ShopLayout from "../routes/shop";

const ROUTES_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../routes");

type RouteModule = { default: ComponentType<{ loaderData: unknown }> };
const MODULES = import.meta.glob<RouteModule>(["../routes/shop.*.tsx"], { eager: true });

/** react-router flat-route file name -> URL pattern (`shop.item.$contractAddress.$itemId_.bid` -> `/shop/item/:contractAddress/:itemId/bid`). */
function patternOf(file: string): string {
  const segments = path.basename(file, ".tsx").split(".").map((s) => s.replace(/_$/, ""));
  return `/${segments.filter((s) => s !== "_index").map((s) => (s === "$" ? "*" : s.startsWith("$") ? `:${s.slice(1)}` : s)).join("/")}`;
}

const SAMPLE: Record<string, string> = {
  contractAddress: "0xabc0000000000000000000000000000000000001",
  itemId: "7",
  tokenId: "5",
  address: "0x1111111111111111111111111111111111111111",
  id: "o1",
  listId: "l1",
};

function urlOf(pattern: string): string {
  return pattern.replace(/:([A-Za-z]+)/g, (_, name: string) => SAMPLE[name]!).replace("*", "not-a-page");
}

function render(pattern: string, url: string, Component: ComponentType<{ loaderData: unknown }>, loaderData: unknown): string {
  const router = createMemoryRouter(
    [
      {
        id: "shop",
        path: "/shop",
        element: <ShopLayout />,
        children: [{ id: "page", path: pattern.slice("/shop/".length), Component: Component as ComponentType, loader: () => loaderData }],
      },
    ],
    { initialEntries: [url], hydrationData: { loaderData: { page: loaderData } } },
  );
  return renderToString(
    <QueryClientProvider client={new QueryClient()}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
}

const byPattern = new Map(Object.entries(MODULES).map(([file, mod]) => [patternOf(file), { file, mod }]));

describe("the /shop routes are ui3's route table", () => {
  it("every Shop page has one sites route at its path, rendering the ui3 module the table names", () => {
    for (const page of SHOP_PAGES) {
      const pattern = page.id === "notFound" ? "/shop/*" : `/shop/${page.path}`;
      const hit = byPattern.get(pattern);
      expect(hit, `${page.id}: no sites route for ${pattern}`).toBeDefined();
      const source = fs.readFileSync(path.join(ROUTES_DIR, path.basename(hit!.file)), "utf8");
      const module = page.module.replace(/^\.\//, "").replace(/\.tsx$/, "");
      const onDisk = fs.existsSync(path.resolve(ROUTES_DIR, "../../../../ui3/src/shop", page.module));
      if (onDisk) expect(source, `${page.id} renders @ui/shop/${module}`).toContain(`from "@ui/shop/${module}"`);
      else expect(source, `${page.id} renders the table's placeholder until ${page.module} exists`).toContain(`p.id === "${page.id}"`);
    }
    const expected = new Set([...SHOP_PAGES.map((p) => (p.id === "notFound" ? "/shop/*" : `/shop/${p.path}`)), "/shop"]);
    expect([...byPattern.keys()].filter((p) => !expected.has(p))).toEqual([]);
  });

  it("every page renders on the server, signed out, with only its loader's head", () => {
    for (const page of SHOP_PAGES) {
      const pattern = page.id === "notFound" ? "/shop/*" : `/shop/${page.path}`;
      const { mod } = byPattern.get(pattern)!;
      const data: ShopPageData = { page: page.id, seo: defaultShopSeo(page.id), queries: [] };
      const html = render(pattern, urlOf(pattern), mod.default, data);
      expect(html, page.id).toContain("shop-chrome");
      expect(html, page.id).toContain("data-site-layout-navigation");
    }
  });
});

describe("server-read answers reach the first paint", () => {
  const C = SAMPLE.contractAddress!;
  const fetchImpl = (async (input: RequestInfo | URL) => {
    const url = new URL(String(input));
    const bodies: Record<string, unknown> = {
      "/shop-api/v1/items": { data: [{ name: "Pirate Hat", thumbnail: "https://img.test/hat.png", creator: "0x1", rarity: "rare", category: "wearable", chainId: 137, network: "MATIC", data: { wearable: { description: "A hat for pirates." } } }], total: 1 },
      "/shop-api/v1/collections": { data: [{ contractAddress: C, name: "Sea Things", creator: "0x1" }] },
      "/shop-api/v3/catalog/unified": { data: [], total: 0 },
    };
    const body = bodies[url.pathname];
    return body ? Response.json(body) : Response.json({ error: "not found" }, { status: 404 });
  }) as typeof fetch;
  const client = createShopClient(hostApiOptions({ base: "http://node.test", signer: null, fetchImpl }));

  it("the item page's SSR HTML shows the primed name, description and collection", async () => {
    const pattern = "/shop/item/:contractAddress/:itemId";
    const { mod } = byPattern.get(pattern)!;
    const url = urlOf(pattern);
    const { data } = await loadShopPage("itemDetail", { request: new Request(`https://sites.test${url}`), params: { contractAddress: C, itemId: "7" }, client, env: {} });
    const html = render(pattern, url, mod.default, data);
    expect(html).toContain("Pirate Hat");
    expect(html).toContain("A hat for pirates.");
    expect(html).toContain("Sea Things");

    const unprimed = render(pattern, url, mod.default, { ...data, queries: [] });
    expect(unprimed).not.toContain("Pirate Hat");
  });
});

describe("Shop rollout data during server rendering", () => {
  function FlagProbe() {
    const deals = useRemoteFlag(REMOTE_FLAG.creatorSales);
    const store = useMyStoreAccess(SAMPLE.address ?? null, false);
    return <p>{`deals=${deals}; store=${store}`}</p>;
  }

  it("renders each request's switches and variants without seeding the browser store", () => {
    const url = "/shop/overview";
    const common: ShopPageData = { page: "overview", seo: defaultShopSeo("overview"), queries: [] };
    const allowed: ShopPageData = {
      ...common,
      featureFlags: { [`dapps-${REMOTE_FLAG.creatorSales}`]: true, [`dapps-${REMOTE_FLAG.myStore}`]: true },
      featureFlagVariants: { [`dapps-${REMOTE_FLAG.myStore}`]: SAMPLE.address! },
    };
    const denied: ShopPageData = {
      ...common,
      featureFlags: { [`dapps-${REMOTE_FLAG.creatorSales}`]: false, [`dapps-${REMOTE_FLAG.myStore}`]: true },
      featureFlagVariants: { [`dapps-${REMOTE_FLAG.myStore}`]: "0x2222222222222222222222222222222222222222" },
    };

    expect(render(url, url, FlagProbe, allowed)).toContain("deals=true; store=on");
    expect(render(url, url, FlagProbe, denied)).toContain("deals=false; store=off");
    expect(render(url, url, FlagProbe, allowed)).toContain("deals=true; store=on");
    expect(remoteFlagVariant(REMOTE_FLAG.myStore)).toBeNull();
  });
});
