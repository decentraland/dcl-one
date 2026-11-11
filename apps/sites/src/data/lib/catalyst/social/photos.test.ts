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
const identity = { signer: OWNER } as AuthIdentity;
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

beforeEach(() => {
  vi.mocked(getJSON).mockResolvedValue(page([image("a")]));
  vi.mocked(signedGetJSON).mockResolvedValue(page([image("a")]));
});
afterEach(() => vi.resetAllMocks());

describe("fetchUserPhotos", () => {
  it("reads the node's camera reel for the lowercased wallet, one page, unsigned", async () => {
    const result = await fetchUserPhotos(` ${OWNER.toUpperCase().replace("0X", "0x")} `, { limit: 24, offset: 48 });

    expect(getJSON).toHaveBeenCalledWith(PATH, expect.objectContaining({ query: { limit: 24, offset: 48 } }));
    expect(signedGetJSON).not.toHaveBeenCalled();
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
          author: { name: "Alice", address: OWNER },
          people: [{ name: "kurd", address: "0xe5104e449950180c459447f2971c8a760456b635", isGuest: false }],
        },
      ],
      received: 1,
      total: 1,
    });
  });

  it("signs the read with the owner's identity, so private snapshots are listed", async () => {
    vi.mocked(signedGetJSON).mockResolvedValue(page([image("p", { isPublic: false })], 7));

    const result = await fetchUserPhotos(OWNER, { limit: 24 }, { identity });

    expect(signedGetJSON).toHaveBeenCalledWith(PATH, expect.objectContaining({ identity, query: { limit: 24, offset: 0 } }));
    expect(getJSON).not.toHaveBeenCalled();
    expect(result.items[0]?.isPublic).toBe(false);
    expect(result.total).toBe(7);
  });

  it("keeps a row that lacks fields the tab does not read", async () => {
    const legacy = image("old");
    const { placeId: _placeId, ...metadata } = legacy.metadata;
    vi.mocked(getJSON).mockResolvedValue(page([{ ...legacy, metadata: { ...metadata, visiblePeople: [{ userName: "", userAddress: "0x1", isGuest: true }] } }]));

    const result = await fetchUserPhotos(OWNER, { limit: 24 });

    expect(result.items.map((photo) => photo.id)).toEqual(["old"]);
    expect(result.items[0]?.people).toEqual([{ name: "", address: "0x1", isGuest: true }]);
  });

  it("leaves out a row that does not parse or whose image is not http(s), and still counts it as received", async () => {
    vi.mocked(getJSON).mockResolvedValue(
      page([image("a"), { id: "broken" }, image("js", { url: "javascript:alert(1)" }), image("b")], 9),
    );

    const result = await fetchUserPhotos(OWNER, { limit: 24 });

    expect(result.items.map((photo) => photo.id)).toEqual(["a", "b"]);
    expect(result.received).toBe(4);
    expect(result.total).toBe(9);
    expect(warnInvalid).toHaveBeenCalledTimes(2);
  });

  it("falls back to the full image when the thumbnail is not http(s)", async () => {
    vi.mocked(getJSON).mockResolvedValue(page([image("a", { thumbnailUrl: "" })]));

    const [photo] = (await fetchUserPhotos(OWNER, { limit: 24 })).items;

    expect(photo?.thumbnailUrl).toBe("https://reel.example/a.jpg");
  });

  it.each([
    ["no images array", { currentImages: 1 }],
    ["no total", { images: [] }],
    ["not an object", "nope"],
  ])("throws a typed error when the response has %s", async (_case, body) => {
    vi.mocked(getJSON).mockResolvedValue(body);

    await expect(fetchUserPhotos(OWNER, { limit: 24 })).rejects.toBeInstanceOf(CatalystError);
  });

  it("leaves out a row whose scene has no location", async () => {
    const row = image("a");
    vi.mocked(getJSON).mockResolvedValue(page([{ ...row, metadata: { ...row.metadata, scene: { name: "Plaza" } } }, image("b")]));

    const result = await fetchUserPhotos(OWNER, { limit: 24 });

    expect(result.items.map((photo) => photo.id)).toEqual(["b"]);
    expect(warnInvalid).toHaveBeenCalledTimes(1);
  });

  it("passes a failed signed read through", async () => {
    vi.mocked(signedGetJSON).mockRejectedValue(new CatalystError("Catalyst returned 401", PATH, 401));

    await expect(fetchUserPhotos(OWNER, { limit: 24 }, { identity })).rejects.toMatchObject({ status: 401 });
  });

  it("passes a failed read through", async () => {
    vi.mocked(getJSON).mockRejectedValue(new CatalystError("Catalyst returned 503", PATH, 503));

    await expect(fetchUserPhotos(OWNER, { limit: 24 })).rejects.toMatchObject({ status: 503 });
  });
});

describe("fetchPhoto", () => {
  it("reads one snapshot's metadata by id", async () => {
    vi.mocked(getJSON).mockResolvedValue(image("x"));

    const photo = await fetchPhoto(ID.toUpperCase());

    expect(getJSON).toHaveBeenCalledWith(`/camera-reel-service/api/images/${ID}/metadata`, expect.anything());
    expect(photo?.id).toBe("x");
  });

  it.each(["..", "", "x/../../users", `${ID}/x`])("never requests an id that is not one: %j", async (id) => {
    expect(await fetchPhoto(id)).toBeNull();
    expect(getJSON).not.toHaveBeenCalled();
  });

  it("is null for a snapshot the service does not have, or cannot show", async () => {
    vi.mocked(getJSON).mockRejectedValueOnce(new CatalystError("Catalyst returned 404", "/x", 404)).mockResolvedValueOnce({ id: "broken" });

    expect(await fetchPhoto(ID)).toBeNull();
    expect(await fetchPhoto(ID)).toBeNull();
  });

  it("passes any other failure through", async () => {
    vi.mocked(getJSON).mockRejectedValue(new CatalystError("Catalyst returned 503", "/x", 503));

    await expect(fetchPhoto(ID)).rejects.toMatchObject({ status: 503 });
  });
});

describe("formatPhotoDate", () => {
  it("reads Unix seconds and ISO dates as the profile site prints them", () => {
    expect(formatPhotoDate("1794327859")).toBe(new Date(1794327859 * 1000).toLocaleDateString("en-US", { month: "long", day: "2-digit", year: "numeric" }));
    expect(formatPhotoDate("2026-11-08T12:00:00Z")).toBe("November 08, 2026");
    expect(formatPhotoDate(" 1794327859 ")).toBe(formatPhotoDate("1794327859"));
  });

  it("prints the UTC day whatever the browser's time zone, as the server renders it", () => {
    const zone = process.env.TZ;
    process.env.TZ = "Asia/Tokyo";
    try {
      expect(formatPhotoDate("2026-11-08T23:30:00Z")).toBe("November 08, 2026");
    } finally {
      process.env.TZ = zone;
    }
  });

  it.each(["", "   ", "not a date"])("is empty for %j", (value) => {
    expect(formatPhotoDate(value)).toBe("");
  });
});
