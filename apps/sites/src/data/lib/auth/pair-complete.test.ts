import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { completePairing } from "./pair-complete";

/** A response whose body records whether it was released without being read; `closed` lets a reader finish it. */
function answer(status: number, text = "", closed = false) {
  const released = vi.fn();
  const body = new ReadableStream<Uint8Array>({
    start(controller) {
      if (text) controller.enqueue(new TextEncoder().encode(text));
      if (closed) controller.close();
    },
    cancel: released,
  });
  return { response: new Response(body, { status }), released };
}

describe("when the phone completes a pairing", () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("should post the signature for the session", async () => {
    fetchMock.mockResolvedValue(answer(200, "{}").response);
    await completePairing("session-id", "0xabc", "0xsig");
    expect(fetchMock).toHaveBeenCalledWith("/internal/pair", expect.objectContaining({
      method: "POST",
      body: JSON.stringify({ kind: "complete", id: "session-id", signer: "0xabc", signature: "0xsig" }),
    }));
  });

  it("should release the body of a successful answer", async () => {
    const { response, released } = answer(200, "{\"ok\":true}");
    fetchMock.mockResolvedValue(response);
    await completePairing("session-id", "0xabc", "0xsig");
    expect(released).toHaveBeenCalledOnce();
  });

  describe.each([
    [409, "This code was already used."],
    [410, "This code expired \u{2014} scan a fresh one."],
    [404, "This code expired \u{2014} scan a fresh one."],
  ])("and the server answers %i", (status, message) => {
    it("should release the body and say why", async () => {
      const { response, released } = answer(status, "{\"error\":\"gone\"}");
      fetchMock.mockResolvedValue(response);
      await expect(completePairing("session-id", "0xabc", "0xsig")).rejects.toThrow(message);
      expect(released).toHaveBeenCalledOnce();
    });
  });

  describe("and the server fails with an explanation", () => {
    it("should read it and show it", async () => {
      fetchMock.mockResolvedValue(answer(400, "{\"error\":\"Signature does not match\"}", true).response);
      await expect(completePairing("session-id", "0xabc", "0xsig")).rejects.toThrow("Signature does not match");
    });
  });

  describe("and the server fails with an unreadable body", () => {
    it("should fall back to the status", async () => {
      fetchMock.mockResolvedValue(answer(502, "<html>", true).response);
      await expect(completePairing("session-id", "0xabc", "0xsig")).rejects.toThrow("The sign-in couldn't be completed (502).");
    });
  });

  describe("and the request never reaches the server", () => {
    it("should pass the network failure on", async () => {
      fetchMock.mockRejectedValue(new TypeError("Failed to fetch"));
      await expect(completePairing("session-id", "0xabc", "0xsig")).rejects.toThrow("Failed to fetch");
    });
  });
});
