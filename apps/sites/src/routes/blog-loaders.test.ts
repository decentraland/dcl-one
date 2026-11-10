import { beforeEach, describe, expect, it, vi } from "vitest";
import { createFixtureBlogSource } from "@/data/lib/blog/fixture-source";
import { currentBlogSource } from "@/data/lib/blog/current-source";
import { blogKeys } from "@/data/lib/blog/keys";
import { loader as postLoader } from "./routes/blog.$categorySlug.$postSlug";
import { loader as searchLoader } from "./routes/blog.search";
import type { HomeData } from "./lib/blog-home";
vi.mock("@/data/lib/blog/current-source", () => ({
  currentBlogSource: vi.fn(),
}));
function payload(value: unknown): HomeData {
  return (value as { data: HomeData }).data;
}
describe("when the document loader serves an article or search", () => {
  let request: Request;
  beforeEach(() => {
    request = new Request("https://sites.example/blog");
    vi.mocked(currentBlogSource).mockResolvedValue(createFixtureBlogSource());
  });
  it("should return 404 for an unknown article", async () => {
    await expect(
      postLoader({
        request,
        params: { categorySlug: "announcements", postSlug: "missing-post" },
      } as never),
    ).rejects.toMatchObject({ status: 404 });
  });
  it("should preload a valid article and search result through the shared source factories", async () => {
    const article = await postLoader({
      request,
      params: {
        categorySlug: "announcements",
        postSlug: "decentralands-2024-recap",
      },
    } as never);
    expect(
      payload(article).dehydratedState.queries.some(
        (query) =>
          JSON.stringify(query.queryKey) ===
          JSON.stringify(
            blogKeys.post("announcements", "decentralands-2024-recap"),
          ),
      ),
    ).toBe(true);
    const search = await searchLoader({
      request: new Request("https://sites.example/blog/search?q=decentraland"),
    } as never);
    expect(
      payload(search).dehydratedState.queries.some(
        (query) =>
          JSON.stringify(query.queryKey) ===
          JSON.stringify(blogKeys.search("decentraland")),
      ),
    ).toBe(true);
  });
});
