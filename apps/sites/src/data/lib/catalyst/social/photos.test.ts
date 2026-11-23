import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../client", async (original) => ({
  ...(await original<typeof import("../client")>()),
  getJSON: vi.fn(),
  signedGetJSON: vi.fn(),
}));
vi.mock("../warn", () => ({ warnInvalid: vi.fn() }));

import type { AuthIdentity } from "../../auth/types";
import { CatalystError, getJSON, signedGetJSON } from "../client";
import { warnInvalid } from "../warn";
import { fetchPhoto, fetchUserPhotos, formatPhotoDate } from "./photos";

const OWNER = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const PATH = `/camera-reel-service/api/users/${OWNER}/images`;
const ID = "7f554ad3-ea9c-4153-89cd-5b1f6c03bfbb";

function image(id: string, overrides: Record<string, unknown> = {}) {
  return {
    id,
    url: `https://reel.example/${id}.jpg`,
    thumbnailUrl: `https://reel.example/${id}-thumbnail.jpg`,
    isPublic: true,
    metadata: {
      userName: " Alice ",
      userAddress: OWNER.toUpperCase().replace("0X", "0x"),
      dateTime: "1794327859",
      realm: "main",
      scene: { name: "Genesis Plaza", location: { x: "0", y: "1" } },
      visiblePeople: [
        { userName: "kurd", userAddress: "0xE5104E449950180C459447F2971C8A760456B635", wearables: [], isGuest: false, isEmoting: null },
      ],
      placeId: "place-1",
    },
    ...overrides,
  };
}

const page = (images: unknown[], currentImages = images.length) => ({ images, currentImages, maxImages: 500 });

let identity: AuthIdentity;

beforeEach(() => {
  identity = { signer: OWNER } as AuthIdentity;
});
afterEach(() => {
  vi.resetAllMocks();
  vi.unstubAllEnvs();
});

describe("fetchUserPhotos", () => {
  let result: Awaited<ReturnType<typeof fetchUserPhotos>>;

  beforeEach(() => {
    vi.mocked(getJSON).mockResolvedValue(page([image("a")]));
    vi.mocked(signedGetJSON).mockResolvedValue(page([image("a")]));
  });

  describe("when the read is unsigned", () => {
    beforeEach(async () => {
      result = await fetchUserPhotos(` ${OWNER.toUpperCase().replace("0X", "0x")} `, { limit: 24, offset: 48 });
    });

    it("should read one page of the node's camera reel for the lowercased wallet", () => {
      expect(getJSON).toHaveBeenCalledWith(PATH, expect.objectContaining({ query: { limit: 24, offset: 48 } }));
    });

    it("should not sign the read", () => {
      expect(signedGetJSON).not.toHaveBeenCalled();
    });

    it("should map the page's photos", () => {
      expect(result).toEqual({
        items: [
          {
            id: "a",
            url: "https://reel.example/a.jpg",
            thumbnailUrl: "https://reel.example/a-thumbnail.jpg",
            isPublic: true,
            dateTime: "1794327859",
            realm: "main",
            scene: { name: "Genesis Plaza", x: "0", y: "1" },
            // The service's own spelling is kept beside the lowercased address when it differs.
            author: { name: "Alice", address: OWNER, sourceAddress: OWNER.toUpperCase().replace("0X", "0x") },
            people: [
              {
                name: "kurd",
                address: "0xe5104e449950180c459447f2971c8a760456b635",
                sourceAddress: "0xE5104E449950180C459447F2971C8A760456B635",
                isGuest: false,
              },
            ],
          },
        ],
        received: 1,
        total: 1,
      });
    });
  });

  describe("when the owner's identity is given", () => {
    beforeEach(async () => {
      vi.mocked(signedGetJSON).mockResolvedValueOnce(page([image("p", { isPublic: false })], 7));
      result = await fetchUserPhotos(OWNER, { limit: 24 }, { identity });
    });

    it("should sign the read with it", () => {
      expect(signedGetJSON).toHaveBeenCalledWith(PATH, expect.objectContaining({ identity, query: { limit: 24, offset: 0 } }));
    });

    it("should not make an unsigned read", () => {
      expect(getJSON).not.toHaveBeenCalled();
    });

    it("should list private snapshots", () => {
      expect(result.items[0]?.isPublic).toBe(false);
    });

    it("should answer the reel's total", () => {
      expect(result.total).toBe(7);
    });
  });

  describe("when a row lacks fields the tab does not read", () => {
    beforeEach(async () => {
      const legacy = image("old");
      const { placeId: _placeId, ...metadata } = legacy.metadata;
      vi.mocked(getJSON).mockResolvedValueOnce(page([{ ...legacy, metadata: { ...metadata, visiblePeople: [{ userName: "", userAddress: "0x1", isGuest: true }] } }]));
      result = await fetchUserPhotos(OWNER, { limit: 24 });
    });

    it("should keep the row", () => {
      expect(result.items.map((photo) => photo.id)).toEqual(["old"]);
    });

    it("should keep its people", () => {
      expect(result.items[0]?.people).toEqual([{ name: "", address: "0x1", isGuest: true }]);
    });
  });

  describe("when a row does not parse or its image is not http(s)", () => {
    beforeEach(async () => {
      vi.mocked(getJSON).mockResolvedValueOnce(
        page([image("a"), { id: "broken" }, image("js", { url: "javascript:alert(1)" }), image("b")], 9),
      );
      result = await fetchUserPhotos(OWNER, { limit: 24 });
    });

    it("should leave it out", () => {
      expect(result.items.map((photo) => photo.id)).toEqual(["a", "b"]);
    });

    it("should still count it as received", () => {
      expect(result.received).toBe(4);
    });

    it("should answer the reel's total", () => {
      expect(result.total).toBe(9);
    });

    it("should warn about each invalid row", () => {
      expect(warnInvalid).toHaveBeenCalledTimes(2);
    });
  });

  describe("when the thumbnail is not http(s)", () => {
    beforeEach(() => {
      vi.mocked(getJSON).mockResolvedValueOnce(page([image("a", { thumbnailUrl: "" })]));
    });

    it("should fall back to the full image", async () => {
      const [photo] = (await fetchUserPhotos(OWNER, { limit: 24 })).items;
      expect(photo?.thumbnailUrl).toBe("https://reel.example/a.jpg");
    });
  });

  describe.each([
    ["no images array", { currentImages: 1 }],
    ["no total", { images: [] }],
    ["not an object", "nope"],
  ])("when the response has %s", (_case, body) => {
    beforeEach(() => {
      vi.mocked(getJSON).mockResolvedValue(body);
    });

    it("should throw a typed error", async () => {
      await expect(fetchUserPhotos(OWNER, { limit: 24 })).rejects.toBeInstanceOf(CatalystError);
    });

    it("should say the response is not a gallery", async () => {
      await expect(fetchUserPhotos(OWNER, { limit: 24 })).rejects.toThrow("the response did not have the gallery shape");
    });
  });

  describe("when a row's scene has no location", () => {
    beforeEach(async () => {
      const row = image("a");
      vi.mocked(getJSON).mockResolvedValueOnce(page([{ ...row, metadata: { ...row.metadata, scene: { name: "Plaza" } } }, image("b")]));
      result = await fetchUserPhotos(OWNER, { limit: 24 });
    });

    it("should leave it out", () => {
      expect(result.items.map((photo) => photo.id)).toEqual(["b"]);
    });

    it("should warn about it", () => {
      expect(warnInvalid).toHaveBeenCalledTimes(1);
    });
  });

  describe("when the signed read fails", () => {
    beforeEach(() => {
      vi.mocked(signedGetJSON).mockRejectedValueOnce(new CatalystError("Catalyst returned 401", PATH, 401));
    });

    it("should pass the failure through", async () => {
      await expect(fetchUserPhotos(OWNER, { limit: 24 }, { identity })).rejects.toMatchObject({ status: 401, message: "Catalyst returned 401" });
    });
  });

  describe("when the read fails", () => {
    beforeEach(() => {
      vi.mocked(getJSON).mockRejectedValueOnce(new CatalystError("Catalyst returned 503", PATH, 503));
    });

    it("should pass the failure through", async () => {
      await expect(fetchUserPhotos(OWNER, { limit: 24 })).rejects.toMatchObject({ status: 503, message: "Catalyst returned 503" });
    });
  });
});

describe("fetchPhoto", () => {
  describe("when the id is a snapshot id", () => {
    let photo: Awaited<ReturnType<typeof fetchPhoto>>;

    beforeEach(async () => {
      vi.mocked(getJSON).mockResolvedValueOnce(image("x"));
      photo = await fetchPhoto(ID.toUpperCase());
    });

    it("should read the snapshot's metadata by its lowercased id", () => {
      expect(getJSON).toHaveBeenCalledWith(`/camera-reel-service/api/images/${ID}/metadata`, expect.anything());
    });

    it("should answer the snapshot", () => {
      expect(photo?.id).toBe("x");
    });
  });

  describe.each(["..", "", "x/../../users", `${ID}/x`])("when the id %j is not one", (id) => {
    it("should answer null", async () => {
      expect(await fetchPhoto(id)).toBeNull();
    });

    it("should never request it", async () => {
      await fetchPhoto(id);
      expect(getJSON).not.toHaveBeenCalled();
    });
  });

  describe("when the service does not have the snapshot", () => {
    beforeEach(() => {
      vi.mocked(getJSON).mockRejectedValueOnce(new CatalystError("Catalyst returned 404", "/x", 404));
    });

    it("should be null", async () => {
      expect(await fetchPhoto(ID)).toBeNull();
    });
  });

  describe("when the service cannot show the snapshot", () => {
    beforeEach(() => {
      vi.mocked(getJSON).mockResolvedValueOnce({ id: "broken" });
    });

    it("should be null", async () => {
      expect(await fetchPhoto(ID)).toBeNull();
    });
  });

  describe("when the read fails otherwise", () => {
    beforeEach(() => {
      vi.mocked(getJSON).mockRejectedValueOnce(new CatalystError("Catalyst returned 503", "/x", 503));
    });

    it("should pass the failure through", async () => {
      await expect(fetchPhoto(ID)).rejects.toMatchObject({ status: 503, message: "Catalyst returned 503" });
    });
  });
});

describe("formatPhotoDate", () => {
  describe("when the date is in Unix seconds", () => {
    it("should print it as the profile site does", () => {
      expect(formatPhotoDate("1794327859")).toBe(new Date(1794327859 * 1000).toLocaleDateString("en-US", { month: "long", day: "2-digit", year: "numeric" }));
    });

    describe("and it has surrounding spaces", () => {
      it("should print it the same", () => {
        expect(formatPhotoDate(" 1794327859 ")).toBe(formatPhotoDate("1794327859"));
      });
    });
  });

  describe("when the date is ISO", () => {
    it("should print it as the profile site does", () => {
      expect(formatPhotoDate("2026-11-08T12:00:00Z")).toBe("November 08, 2026");
    });
  });

  describe("when the browser's time zone is ahead of UTC", () => {
    beforeEach(() => {
      vi.stubEnv("TZ", "Asia/Tokyo");
    });

    it("should print the UTC day, as the server renders it", () => {
      expect(formatPhotoDate("2026-11-08T23:30:00Z")).toBe("November 08, 2026");
    });
  });

  describe("when the date is empty or invalid", () => {
    it.each(["", "   ", "not a date"])("should be empty for %j", (value) => {
      expect(formatPhotoDate(value)).toBe("");
    });
  });
});
