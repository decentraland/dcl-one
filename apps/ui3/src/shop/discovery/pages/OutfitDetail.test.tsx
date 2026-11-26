
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createShopClient } from "@shop/api/shop";
import { shopCart } from "../../cart/store";
import { catalogClient, catalogTransport, COLLECTION, defaultAnswers, ShopHarness, type CatalogAnswers, type CatalogScenario } from "../fixtures";
import OutfitDetail, { readEmoteDefinition } from "./OutfitDetail";

const OUTFIT = {
  id: "0f1e2d3c-4b5a-4968-8778-695a4b3c2d1e",
  name: "Nacho Night",
  thumbnailHash: "",
  items: [
    { contractAddress: COLLECTION, itemId: "7" },
    { contractAddress: COLLECTION, itemId: "8" },
  ],
  bodyShape: "unisex",
  gradientFrom: "#3a0653",
  gradientTo: "#175e52",
  published: true,
};

const preview = vi.hoisted(() => ({
  emote: null as null | ((playback: { base64: string; loop: boolean } | null) => void),
  // A read that only ends when it is aborted, as fetchEmoteBase64 does then: with null.
  stall: false,
  aborted: false,
}));
vi.mock("../../detail/lib/pickRenderer", () => ({ pickRenderer: () => ({ renderer: "full", reason: "connection-ok" }) }));
vi.mock("../../detail/lib/emoteBase64", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../detail/lib/emoteBase64")>()),
  fetchEmoteBase64: (_urn: string, _peer: string | undefined, signal?: AbortSignal) =>
    new Promise((resolve) => {
      preview.emote = resolve;
      if (preview.stall) signal?.addEventListener("abort", () => ((preview.aborted = true), resolve(null)));
    }),
}));
vi.mock("../../../wearable-preview/WearablePreview", () => ({
  default: () => {
    throw new Error("the outfit page must not draw the three.js scene");
  },
}));

beforeEach(() => {
  preview.emote = null;
  preview.stall = false;
  preview.aborted = false;
  vi.stubGlobal("fetch", vi.fn(async () => new Response("{}", { status: 200 })));
  shopCart.clear();
});
afterEach(() => vi.unstubAllGlobals());

function outfit(overrides: CatalogAnswers = { [`/v1/outfits/${OUTFIT.id}`]: { outfit: OUTFIT } }, scenario: CatalogScenario = "ok", legacy = false) {
  const client = legacy
    ? createShopClient({ base: "http://shop.test", mode: "legacy", transport: catalogTransport(scenario, overrides), signer: null })
    : catalogClient(scenario, overrides);
  render(
    <ShopHarness path="/shop/items/outfits/:id" entry={`/shop/items/outfits/${OUTFIT.id}`} client={client} session="web2">
      <OutfitDetail />
    </ShopHarness>,
  );
}

describe("an outfit", () => {
  it("loading shows a busy page", () => {
    outfit(undefined, "loading");
    expect(screen.getByTestId("outfit-detail-loading").getAttribute("aria-busy")).toBe("true");
  });

  it("lists its items as rows and adds the look as one line per item", async () => {
    outfit();
    expect(await screen.findByRole("heading", { level: 1, name: "Nacho Night" })).toBeTruthy();
    expect(screen.getByTestId("outfit-detail-meta")).toHaveTextContent("2 items in this outfit");
    const list = screen.getByTestId("outfit-items");
    await waitFor(() => expect(within(list).getAllByRole("link", { name: /.+/ }).length).toBeGreaterThanOrEqual(2));
    expect(within(list).getAllByTestId("outfit-detail-item")).toHaveLength(2);
    await waitFor(() => expect(screen.getByTestId("outfit-detail-cta")).toHaveTextContent("Add 2 items to cart"));
    fireEvent.click(screen.getByTestId("outfit-detail-cta"));
    await waitFor(() => expect(shopCart.getSnapshot().lines).toHaveLength(2));
    const lines = shopCart.getSnapshot().lines;
    expect(lines.every((l) => l.provenance.outfitId === OUTFIT.id && l.method === null)).toBe(true);
    await waitFor(() => expect(screen.getByTestId("outfit-detail-cta")).toHaveTextContent("Added to cart"));
  });

  it("adds creators' primary sales only without the Marketplace, never a resale in their place", async () => {
    const asked: (string | null)[] = [];
    const answers = defaultAnswers();
    outfit({
      [`/v1/outfits/${OUTFIT.id}`]: { outfit: OUTFIT },
      "/v3/catalog/unified": (url: URL) => {
        if (url.searchParams.get("itemId")) asked.push(url.searchParams.get("listingType"));
        return answers["/v3/catalog/unified"]!(url);
      },
    });
    await waitFor(() => expect(screen.getByTestId("outfit-detail-cta")).toHaveTextContent("Add 2 items to cart"));
    asked.length = 0;
    fireEvent.click(screen.getByTestId("outfit-detail-cta"));
    await waitFor(() => expect(asked).toHaveLength(2));
    expect(asked.every((t) => t === "primary")).toBe(true);
  });

  it("an unknown outfit is not found", async () => {
    outfit({});
    expect(await screen.findByTestId("outfit-notfound")).toHaveTextContent("This outfit isn't available");
  });

  it("a failed read says so with a retry", async () => {
    outfit(undefined, "error");
    expect(await screen.findByTestId("outfit-detail-error")).toHaveTextContent("Something went wrong. Please try again.");
  });

  it("a node without the outfits service (legacy mode: no alias) says so instead of failing", async () => {
    outfit(undefined, "ok", true);
    expect(await screen.findByTestId("outfit-unavailable")).toHaveTextContent("Outfits are not served by this node yet.");
  });
});

describe("the outfit's 3D preview", () => {
  it("waits for the outfit's emote, then mounts once with it sent whole and the default emote off for a play-once clip", async () => {
    vi.stubGlobal("WebGLRenderingContext", class {});
    outfit();
    await waitFor(() => expect(preview.emote).not.toBeNull());
    expect(screen.queryByTitle("3D preview")).toBeNull();

    act(() => preview.emote!({ base64: "eyJlbW90ZSI6dHJ1ZX0=", loop: false }));
    const frame = (await screen.findByTitle("3D preview")) as HTMLIFrameElement;
    const params = new URL(frame.src).searchParams;
    const urns = params.getAll("urn");
    expect(urns).toHaveLength(2);
    expect(urns.every((urn) => urn.startsWith(`urn:decentraland:matic:collections-v2:${COLLECTION}:`))).toBe(true);
    expect(params.getAll("base64")).toEqual(["eyJlbW90ZSI6dHJ1ZX0="]);
    expect(params.has("emote")).toBe(false);
    expect(params.get("disableDefaultEmotes")).toBe("true");
    expect(params.get("profile")).toBe("default");
  });

  it("does not wait on an emote definition that never comes: past its deadline the outfit mounts with the emote's urn", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      vi.stubGlobal("WebGLRenderingContext", class {});
      preview.stall = true;
      outfit();
      await waitFor(() => expect(preview.emote).not.toBeNull());
      expect(screen.queryByTitle("3D preview")).toBeNull();

      await act(async () => {
        await vi.advanceTimersByTimeAsync(5_000);
      });
      expect(preview.aborted).toBe(true);
      const frame = (await screen.findByTitle("3D preview")) as HTMLIFrameElement;
      const params = new URL(frame.src).searchParams;
      expect(params.getAll("urn")).toHaveLength(2);
      expect(params.has("base64")).toBe(false);
      expect(params.has("disableDefaultEmotes")).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("reading an outfit emote's definition", () => {
  it("fails past its deadline rather than answer null, so the query keeps nothing for the next visit", async () => {
    vi.useFakeTimers();
    try {
      preview.stall = true;
      const read = readEmoteDefinition("urn:decentraland:matic:collections-v2:0x0:8", new AbortController().signal);
      const settled = read.then(
        () => null,
        (error: unknown) => error,
      );
      await vi.advanceTimersByTimeAsync(5_000);
      const error = (await settled) as Error | null;
      expect(preview.aborted).toBe(true);
      expect(error).toBeInstanceOf(Error);
      // The query waits on an AbortError instead of ending (query.ts): this must not be one.
      expect(error!.name).not.toBe("AbortError");
    } finally {
      vi.useRealTimers();
    }
  });

  it("answers what the read answers when it is in time, and null when the query itself cancels it", async () => {
    const read = readEmoteDefinition("urn:decentraland:matic:collections-v2:0x0:8", new AbortController().signal);
    await waitFor(() => expect(preview.emote).not.toBeNull());
    preview.emote!({ base64: "e30=", loop: true });
    await expect(read).resolves.toEqual({ base64: "e30=", loop: true });

    preview.stall = true;
    const query = new AbortController();
    const cancelled = readEmoteDefinition("urn:decentraland:matic:collections-v2:0x0:8", query.signal);
    query.abort();
    await expect(cancelled).resolves.toBeNull();
  });
});
