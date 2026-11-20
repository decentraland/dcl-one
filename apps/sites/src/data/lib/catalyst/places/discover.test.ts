import { dehydrate, QueryClient } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { destination } from "./destination.fixture";
import type { Destination, DestinationPage } from "./destinations";
import {
  browsePageRows,
  effectiveSearch,
  fetchPlaceCreators,
  nextBrowseOffset,
  parseCategory,
  parseSection,
  placeCreatorAddress,
  placeCreatorsQuery,
  placesFromDehydrated,
  placesKeys,
  selectLiveRail,
} from "./discover";

function page(data: Destination[], total = data.length, received = data.length): DestinationPage {
  return { data, total, received, copy: null };
}

describe("when parsing the /places URL", () => {
  describe("and reading the category", () => {
    it("should accept a known category case-insensitively", () => {
      expect(parseCategory(" Music ")).toBe("music");
    });

    it("should accept Featured case-insensitively", () => {
      expect(parseCategory("FEATURED")).toBe("featured");
    });

    it("should read an unknown category as All", () => {
      expect(parseCategory("casino")).toBe("all");
    });

    it("should read a missing category as All", () => {
      expect(parseCategory(null)).toBe("all");
    });
  });

  describe("and reading the section", () => {
    it("should read the British spelling of favourites", () => {
      expect(parseSection("favourites")).toBe("favourites");
    });

    it("should read the American spelling of favourites", () => {
      expect(parseSection("favorites")).toBe("favourites");
    });

    it("should read my places", () => {
      expect(parseSection("my")).toBe("my");
    });

    it("should default an unknown tab to Explore all", () => {
      expect(parseSection("admin")).toBe("all");
    });

    it("should default a missing tab to Explore all", () => {
      expect(parseSection(null)).toBe("all");
    });
  });

  describe("and reading the search", () => {
    it("should drop a search shorter than three characters, which places-api rejects", () => {
      expect(effectiveSearch("  ab ")).toBe("");
    });

    it("should keep a trimmed search of three characters", () => {
      expect(effectiveSearch(" abc ")).toBe("abc");
    });

    it("should read a missing search as none", () => {
      expect(effectiveSearch(undefined)).toBe("");
    });
  });
});

describe("when keying the places queries", () => {
  describe("and keying the browse feed", () => {
    let base: ReturnType<typeof placesKeys.browse>;

    beforeEach(() => {
      base = placesKeys.browse({ category: "all", search: "" });
    });

    it("should give every category its own browse entry", () => {
      expect(placesKeys.browse({ category: "music", search: "" })).not.toEqual(base);
    });

    it("should give every search its own browse entry", () => {
      expect(placesKeys.browse({ category: "all", search: "plaza" })).not.toEqual(base);
    });
  });

  describe("and keying member reads", () => {
    it("should put favourites under the private prefix of the lowercased account", () => {
      expect(placesKeys.favourites("0xABC")).toEqual(["private", "0xabc", "places", "favourites"]);
    });

    it("should put my places under the private prefix of the lowercased account", () => {
      expect(placesKeys.mine("0xABC")).toEqual(["private", "0xabc", "places", "mine"]);
    });
  });

  it("should share one creators entry whatever the order or case of the addresses", () => {
    expect(placeCreatorsQuery(["0xB", "0xa", "0xA"]).queryKey).toEqual(placeCreatorsQuery(["0xa", "0xb"]).queryKey);
  });
});

describe("when paging the browse feed", () => {
  describe("and the page was full while the total has more", () => {
    it("should continue", () => {
      expect(nextBrowseOffset(page([], 200, 48), 0)).toBe(48);
    });
  });

  describe("and the page was short even though the total claims more", () => {
    it("should stop", () => {
      expect(nextBrowseOffset(page([], 200, 10), 48)).toBeUndefined();
    });
  });

  describe("and the offset reaches the total", () => {
    it("should stop", () => {
      expect(nextBrowseOffset(page([], 96, 48), 48)).toBeUndefined();
    });
  });
});

describe("when a page lost rows to schema drift", () => {
  it("should continue at the raw page boundary, not at the parsed row count", () => {
    expect(nextBrowseOffset(page([destination()], 200, 48), 48)).toBe(96);
  });
});

describe("when building the grid rows", () => {
  describe("and rows repeat across pages or are unfinished catalogue entries", () => {
    let pages: DestinationPage[];

    beforeEach(() => {
      pages = [
        page([destination({ id: "a" }), destination({ id: "road", title: "Road at 1,2", categories: [] })]),
        page([destination({ id: "a" }), destination({ id: "b" })]),
      ];
    });

    it("should drop them", () => {
      expect(browsePageRows(pages, { category: "all", search: "" }).flat().map((row) => row.id)).toEqual(["a", "b"]);
    });
  });

  describe("and a curated placeholder has no cover", () => {
    let curated: Destination;

    beforeEach(() => {
      curated = destination({ id: "curated", image: null, highlighted: true });
    });

    describe("and the view is Featured", () => {
      it("should retain it", () => {
        expect(
          browsePageRows([page([curated])], { category: "featured", search: "" })
            .flat()
            .map((row) => row.id),
        ).toEqual(["curated"]);
      });
    });

    describe("and the view is the default", () => {
      it("should hide it", () => {
        expect(browsePageRows([page([curated])], { category: "all", search: "" })).toEqual([[]]);
      });
    });
  });

  describe.each([null, "https://example.com/v2/map.png", "https://example.com/bafkreidj26s7aenyxfthfdibnqonzqm5ptc4iamml744gmcyuokewkr76y"])(
    "and a row has the cover %s",
    (image) => {
      let row: Destination;

      beforeEach(() => {
        row = destination({ image });
      });

      it.each([
        { category: "all", search: "plaza" },
        { category: "music", search: "" },
      ] as const)("should preserve an explicit match for %j", (filters) => {
        expect(browsePageRows([page([row])], filters)).toEqual([[row]]);
      });

      it("should hide it on the default view", () => {
        expect(browsePageRows([page([row])], { category: "all", search: "" })).toEqual([[]]);
      });
    },
  );
});

describe("when choosing Live Now", () => {
  describe("and there are more busy scenes than the rail holds", () => {
    let rows: Destination[];

    beforeEach(() => {
      rows = [1, 9, 4, 7, 3].map((count) => destination({ id: `p${count}`, user_count: count }));
      rows.push(destination({ id: "junk", title: "Empty", user_count: 50 }));
    });

    it("should keep the four busiest visible scenes, busiest first", () => {
      expect(selectLiveRail(rows).map((row) => row.id)).toEqual(["p9", "p7", "p4", "p3"]);
    });
  });

  describe("and head counts are empty, missing or negative", () => {
    it("should exclude them by default", () => {
      expect(selectLiveRail([destination({ user_count: 0 }), destination({ user_count: null }), destination({ user_count: -1 })])).toEqual(
        [],
      );
    });
  });

  describe("and a scene is under the minimum head count", () => {
    it("should leave it out", () => {
      expect(selectLiveRail([destination({ user_count: 1 })], 2)).toEqual([]);
    });
  });
});

describe("when crediting a place", () => {
  describe("and it has a deployer and an owner", () => {
    it("should prefer the deployer, lowercased", () => {
      expect(
        placeCreatorAddress({
          creator_address: "0x00000000000000000000000000000000000000BB",
          owner: "0x00000000000000000000000000000000000000aa",
        }),
      ).toBe("0x00000000000000000000000000000000000000bb");
    });
  });

  describe("and the values are not wallets", () => {
    it("should ignore them", () => {
      expect(placeCreatorAddress({ creator_address: "Decentraland", owner: null })).toBeUndefined();
    });
  });
});

describe("when reading creator profiles", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  describe("and there are addresses", () => {
    let fetchMock: ReturnType<typeof vi.fn>;
    let result: Awaited<ReturnType<typeof fetchPlaceCreators>>;

    beforeEach(async () => {
      fetchMock = vi.fn(async () =>
        Response.json([
          { avatars: [{ ethAddress: "0xAA", name: "Ann", avatar: { snapshots: { face256: "https://cdn.example/face.png" } } }] },
        ]),
      );
      vi.stubGlobal("fetch", fetchMock);
      result = await fetchPlaceCreators(["0xaa"]);
    });

    it("should post every address once", () => {
      expect(JSON.parse(String((fetchMock.mock.calls[0] as unknown as [string, RequestInit])[1].body))).toEqual({ ids: ["0xaa"] });
    });

    it("should key the faces by lowercased address", () => {
      expect(result["0xaa"]?.name).toBe("Ann");
    });
  });

  describe("and there are no addresses", () => {
    let fetchMock: ReturnType<typeof vi.fn>;

    beforeEach(() => {
      fetchMock = vi.fn();
      vi.stubGlobal("fetch", fetchMock);
    });

    it("should answer no profiles", async () => {
      await expect(fetchPlaceCreators([])).resolves.toEqual({});
    });

    it("should not call the network", async () => {
      await fetchPlaceCreators([]);
      expect(fetchMock).not.toHaveBeenCalled();
    });
  });

  describe("and the read fails", () => {
    let cancel: ReturnType<typeof vi.spyOn>;
    let result: Promise<unknown>;

    beforeEach(() => {
      const failed = new Response("upstream error", { status: 502 });
      cancel = vi.spyOn(failed.body!, "cancel");
      vi.stubGlobal(
        "fetch",
        vi.fn(async () => failed),
      );
      result = fetchPlaceCreators(["0xaa"]);
    });

    it("should reject so the cards keep their initials", async () => {
      await expect(result).rejects.toThrow("502");
    });

    it("should release the body", async () => {
      await result.catch(() => undefined);
      expect(cancel).toHaveBeenCalledOnce();
    });
  });
});

describe("when the agent markdown reads a dehydrated page", () => {
  describe("and the page carried a browse query", () => {
    let client: QueryClient;

    beforeEach(() => {
      client = new QueryClient();
      client.setQueryData(placesKeys.browse({ category: "all", search: "" }), { pages: [page([destination()])], pageParams: [0] });
    });

    it("should return the first browse page", () => {
      expect(placesFromDehydrated(dehydrate(client)).map((row) => row.id)).toEqual(["plaza"]);
    });
  });

  describe("and the page carried no browse query", () => {
    it("should return nothing", () => {
      expect(placesFromDehydrated(undefined)).toEqual([]);
    });
  });
});
