import { afterEach, describe, expect, it, vi } from "vitest";
import { loader, meta } from "./blog.$slug";

type Result = {
  data: { slug: string; post: { slug: string } | null; related: Record<string, unknown>[]; shareUrl: string };
  init: ResponseInit | null;
};

async function load(slug: string) {
  return (await loader({
    request: new Request(`https://sites.test/blog/${slug}`),
    params: { slug },
  } as never)) as unknown as Result;
}

afterEach(() => {
  vi.unstubAllEnvs();
});

describe("blog post loader", () => {
  it("answers 200 with the post, related posts and an absolute share URL from the request", async () => {
    vi.stubEnv("PUBLIC_CATALYST_URL", "");
    const { data, init } = await load("explore-from-your-browser");
    expect(init?.status).toBe(200);
    expect(data.post?.slug).toBe("explore-from-your-browser");
    expect(data.shareUrl).toBe("https://sites.test/blog/explore-from-your-browser");
    expect(data.related.map((r) => r.slug)).toEqual(["whats-on-live-events-guide", "marketplace-credits-season-one"]);
    expect(data.related[0]).toEqual({
      slug: "whats-on-live-events-guide",
      title: "What's On: A Live Guide to Events Across the World",
      publishedAt: "2026-07-01",
      category: { slug: "platform", title: "Platform" },
      hue: 200,
    });
  });

  it("builds the share URL on the configured public origin, not the request's host", async () => {
    vi.stubEnv("PUBLIC_CATALYST_URL", "https://public.example/");
    const { data } = await load("explore-from-your-browser");
    expect(data.shareUrl).toBe("https://public.example/blog/explore-from-your-browser");
  });

  it("answers 404 with no post for an unknown slug", async () => {
    const { data, init } = await load("nope");
    expect(init?.status).toBe(404);
    expect(data.post).toBeNull();
    expect(data.related).toEqual([]);
    expect(data.shareUrl).toBe("");
    expect(data.slug).toBe("nope");
  });

  it("titles a found post and a missing one", () => {
    expect(
      meta({ loaderData: { post: { title: "T", description: "D" } } } as never),
    ).toEqual([{ title: "T | Decentraland Blog" }, { name: "description", content: "D" }]);
    expect(meta({ loaderData: { post: null } } as never)).toEqual([{ title: "Post not found | Decentraland Blog" }]);
  });
});
