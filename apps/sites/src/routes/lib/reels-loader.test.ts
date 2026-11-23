import { describe, expect, it, vi } from "vitest";

import { CatalystError } from "@/data/lib/catalyst/client";
import type { ProfilePhoto } from "@/data/lib/catalyst/social/photos";

import { headers, meta } from "../routes/reels.$imageId";
import { loadReel, loadReelGallery } from "./reels-loader.server";

const ID = "00000000-0000-0000-0000-000000000001";
const ADDRESS = "0x1111111111111111111111111111111111111111";
const signal = new AbortController().signal;
const photo = { id: ID, isPublic: false } as ProfilePhoto;

describe("Reels loader", () => {
  it("does not request malformed identifiers", async () => {
    const read = vi.fn();
    await expect(loadReel("invalid", signal, read)).rejects.toMatchObject({ status: 404 });
    await expect(loadReelGallery("invalid", signal, read)).rejects.toMatchObject({ status: 404 });
    expect(read).not.toHaveBeenCalled();
  });

  it("opens unlisted photos by exact link without identity and disables document caching", async () => {
    const read = vi.fn().mockResolvedValue(photo);
    const response = await loadReel(ID, signal, read);
    expect(read).toHaveBeenCalledWith(ID, { signal });
    expect(response.data).toEqual(photo);
    const responseHeaders = headers({ loaderHeaders: new Headers(response.init?.headers) });
    expect(Object.fromEntries(responseHeaders)).toEqual({ "cache-control": "no-store", "x-robots-tag": "noindex" });
    expect(meta({ loaderData: photo, matches: [] })).toEqual([
      { title: "Decentraland Reels" },
      { name: "robots", content: "noindex" },
    ]);
  });

  it("keeps missing and unavailable photos distinct", async () => {
    await expect(loadReel(ID, signal, vi.fn().mockResolvedValue(null))).rejects.toMatchObject({ status: 404 });
    await expect(
      loadReel(ID, signal, vi.fn().mockRejectedValue(new CatalystError("unavailable", "/photo", 502))),
    ).rejects.toMatchObject({ status: 503 });
  });

  it("loads only the first public page and omits unlisted or malformed IDs", async () => {
    const publicPhoto = { ...photo, isPublic: true };
    const read = vi.fn().mockResolvedValue({
      items: [photo, publicPhoto, { ...publicPhoto, id: "bad" }],
      received: 3,
      total: 3,
    });
    expect(await loadReelGallery(ADDRESS, signal, read)).toEqual({ photos: [publicPhoto], total: 3 });
    expect(read).toHaveBeenCalledWith(ADDRESS, { limit: 24 }, { signal });
  });

  describe("when Catalyst answers the gallery lookup", () => {
    it("should return an empty gallery for a successful empty page", async () => {
      const read = vi.fn().mockResolvedValue({ items: [], received: 0, total: 0 });
      expect(await loadReelGallery(ADDRESS, signal, read)).toEqual({ photos: [], total: 0 });
    });

    it("should return 503 if every row is invalid", async () => {
      const read = vi.fn().mockResolvedValue({ items: [], received: 1, total: 1 });
      await expect(loadReelGallery(ADDRESS, signal, read)).rejects.toMatchObject({ status: 503 });
    });

    it("should return 503 when Catalyst reports a gallery read failure as 404", async () => {
      const read = vi.fn().mockRejectedValue(new CatalystError("user not found", "/user", 404));
      await expect(loadReelGallery(ADDRESS, signal, read)).rejects.toMatchObject({ status: 503 });
    });
  });

  it("preserves an aborted request", async () => {
    const controller = new AbortController();
    controller.abort();
    const failure = controller.signal.reason;
    const read = vi.fn().mockRejectedValue(failure);
    await expect(loadReel(ID, controller.signal, read)).rejects.toBe(failure);
    await expect(loadReelGallery(ADDRESS, controller.signal, read)).rejects.toBe(failure);
  });
});
