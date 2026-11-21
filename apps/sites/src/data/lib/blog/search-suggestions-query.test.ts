import { QueryClient } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { blogSuggestionKeys, blogSuggestionsQuery } from "./search-suggestions-query";

describe("when requesting Blog suggestions", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("should share the cache key across query casing", () => {
    expect(blogSuggestionsQuery("Dece").queryKey).toEqual(blogSuggestionKeys.query("dece"));
  });

  describe("and the HTTP response failed", () => {
    let response: Response;
    let cancel: ReturnType<typeof vi.spyOn>;
    let result: Promise<unknown>;

    beforeEach(() => {
      response = new Response("{}", { status: 503 });
      cancel = vi.spyOn(response.body!, "cancel");
      vi.stubGlobal("fetch", vi.fn().mockResolvedValueOnce(response));
      result = new QueryClient().fetchQuery(blogSuggestionsQuery("dece"));
    });

    it("should reject with the HTTP status", async () => {
      await expect(result).rejects.toThrow("Blog search suggestions failed: 503");
    });

    it("should cancel the response body", async () => {
      await result.catch(() => undefined);
      expect(cancel).toHaveBeenCalledOnce();
    });
  });

  describe("and the success response is malformed", () => {
    beforeEach(() => {
      vi.stubGlobal("fetch", vi.fn().mockResolvedValueOnce(Response.json({ query: null })));
    });

    it("should reject it", async () => {
      await expect(new QueryClient().fetchQuery(blogSuggestionsQuery("dece"))).rejects.toThrow(/query/);
    });
  });
});
