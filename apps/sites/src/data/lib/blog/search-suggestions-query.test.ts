import { QueryClient } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";

import { blogSuggestionKeys, blogSuggestionsQuery } from "./search-suggestions-query";

afterEach(() => vi.unstubAllGlobals());

describe("when requesting Blog suggestions", () => {
  it("should share the cache key across query casing", () => {
    expect(blogSuggestionsQuery("Dece").queryKey).toEqual(blogSuggestionKeys.query("dece"));
  });

  it("should cancel a failed HTTP response body", async () => {
    const response = new Response("{}", { status: 503 });
    const cancel = vi.spyOn(response.body!, "cancel");
    vi.stubGlobal("fetch", vi.fn().mockResolvedValueOnce(response));
    await expect(new QueryClient().fetchQuery(blogSuggestionsQuery("dece"))).rejects.toThrow("503");
    expect(cancel).toHaveBeenCalledOnce();
  });

  it("should reject malformed success responses", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValueOnce(Response.json({ query: null })));
    await expect(new QueryClient().fetchQuery(blogSuggestionsQuery("dece"))).rejects.toThrow();
  });
});
