import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { fetchDocMarkdown } from "./markdown";

describe("when the Copy action reads a page's Markdown", () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("should give the Markdown text", async () => {
    fetchMock.mockResolvedValue(new Response("# Docs"));
    await expect(fetchDocMarkdown("/docs/index.md")).resolves.toBe("# Docs");
    expect(fetchMock).toHaveBeenCalledWith("/docs/index.md");
  });

  describe("and the server refuses it", () => {
    it("should release the body and fail", async () => {
      const released = vi.fn();
      const body = new ReadableStream<Uint8Array>({
        start(controller) { controller.enqueue(new TextEncoder().encode("Not found")); },
        cancel: released,
      });
      fetchMock.mockResolvedValue(new Response(body, { status: 404 }));
      await expect(fetchDocMarkdown("/docs/missing.md")).rejects.toThrow("Markdown unavailable");
      expect(released).toHaveBeenCalledOnce();
    });
  });

  describe("and the request fails", () => {
    it("should pass the network failure on", async () => {
      fetchMock.mockRejectedValue(new TypeError("Failed to fetch"));
      await expect(fetchDocMarkdown("/docs/index.md")).rejects.toThrow("Failed to fetch");
    });
  });
});
