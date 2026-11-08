// The item page's creator and collection lines: where each links, the creator's name and initial
// fallback, and how the collection collage splits its thumbnails into rows.

import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CREATOR, ShopHarness } from "../../discovery/fixtures";
import { invalidateShopQueries } from "../../discovery/query";
import { DetailCollection, DetailCreator } from "./DetailBadges";

const FACE = "https://profile-images.example/face.png";
const COLLECTION = "0x1111111111111111111111111111111111111111";

beforeEach(() => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input instanceof Request ? input.url : input);
      const body = url.includes("/lambdas/profiles")
        ? [{ avatars: [{ ethAddress: CREATOR, userId: CREATOR, name: "nacho", hasClaimedName: true, avatar: { snapshots: { face256: FACE, body: FACE } } }] }]
        : [];
      return new Response(JSON.stringify(body), { status: 200, headers: { "content-type": "application/json" } });
    }),
  );
  invalidateShopQueries();
});
afterEach(() => vi.unstubAllGlobals());

const settle = () => screen.findByText("Nacho");

describe("DetailCreator", () => {
  it("links to the creator's storefront and reads By {name}", async () => {
    render(
      <ShopHarness>
        <DetailCreator address={CREATOR} />
      </ShopHarness>,
    );
    await settle();
    const link = screen.getByRole("link");
    expect(link.getAttribute("href")).toBe(`/shop/items/creator/${CREATOR}`);
    expect(screen.getByTestId("creator-name")).toHaveTextContent("By Nacho");
    expect(screen.getByTestId("creator-ava").getAttribute("src")).toBe(FACE);
  });

  it("falls back to the coloured initial when the picture fails", async () => {
    render(
      <ShopHarness>
        <DetailCreator address={CREATOR} />
      </ShopHarness>,
    );
    await settle();
    fireEvent.error(screen.getByTestId("creator-ava"));
    expect(screen.getByTestId("creator-ava-letter")).toHaveTextContent("N");
  });

  it("renders nothing without an address", () => {
    const { container } = render(
      <ShopHarness>
        <DetailCreator />
      </ShopHarness>,
    );
    expect(container.querySelector("a")).toBeNull();
  });
});

describe("DetailCollection", () => {
  const badge = (thumbnails: string[]) =>
    render(
      <ShopHarness>
        <DetailCollection contractAddress={COLLECTION} name="goofy hoodies" thumbnails={thumbnails} label={(name) => <>Collection {name}</>} />
      </ShopHarness>,
    );

  it("links to the collection and capitalises its name inside the label", () => {
    badge([]);
    expect(screen.getByRole("link").getAttribute("href")).toBe(`/shop/collection/${COLLECTION}`);
    expect(screen.getByTestId("creator-name")).toHaveTextContent("Collection Goofy hoodies");
    expect(screen.queryByTestId("creator-ava")).toBeNull();
  });

  it("splits up to four thumbnails into two rows, and two or fewer into one full row", () => {
    const { unmount } = badge(["a.png", "b.png", "c.png", "d.png", "e.png"]);
    const rows = screen.getByTestId("creator-ava").children;
    expect(rows).toHaveLength(2);
    expect(rows[0]!.querySelectorAll("img")).toHaveLength(2);
    expect(rows[1]!.querySelectorAll("img")).toHaveLength(2);
    expect(rows[0]!.hasAttribute("data-full")).toBe(false);
    unmount();

    badge(["a.png", "b.png"]);
    const single = screen.getByTestId("creator-ava").children;
    expect(single).toHaveLength(1);
    expect(single[0]!.hasAttribute("data-full")).toBe(true);
  });
});
