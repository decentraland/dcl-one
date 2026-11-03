import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getJSON } from "./client";

describe("when a public API read returns an HTTP error", () => {
  let response: Response;
  let cancel: ReturnType<typeof vi.fn>;
  let fetchImpl: ReturnType<typeof vi.fn<typeof fetch>>;
  beforeEach(() => {
    response = new Response("Unavailable", { status: 503, statusText: "Service Unavailable" });
    cancel = vi.spyOn(response.body!, "cancel");
    fetchImpl = vi.fn<typeof fetch>().mockResolvedValue(response);
  });
  afterEach(() => vi.restoreAllMocks());

  it("should release the response body before surfacing the HTTP error", async () => {
    await expect(getJSON("/places/api/worlds/gallery/scenes", { fetchImpl })).rejects.toMatchObject({ status: 503, message: "Catalyst returned 503 Service Unavailable" });
    expect(cancel).toHaveBeenCalledTimes(1);
  });

  describe("and body cancellation fails", () => {
    beforeEach(() => { cancel.mockRejectedValueOnce(new Error("Already closed")); });
    it("should retain the original HTTP error", async () => {
      await expect(getJSON("/places/api/worlds/gallery/scenes", { fetchImpl })).rejects.toMatchObject({ status: 503 });
    });
  });
});
