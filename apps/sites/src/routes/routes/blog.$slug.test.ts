import { describe, expect, it } from "vitest";
import { loader, meta } from "./blog.$slug";

type Result = {
  data: { slug: string; post: { slug: string } | null; related: { slug: string; hue: number }[]; shareUrl: string };
  init: ResponseInit | null;
};

async function load(slug: string) {
  return (await loader({
    request: new Request(`https://sites.test/blog/${slug}`),
    params: { slug },
  } as never)) as unknown as Result;
}

describe("blog post loader", () => {
  it("answers 200 with the post, related posts and an absolute share URL from the request", async () => {
    const { data, init } = await load("explore-from-your-browser");
    expect(init?.status).toBe(200);
    expect(data.post?.slug).toBe("explore-from-your-browser");
    expect(data.shareUrl).toBe("https://sites.test/blog/explore-from-your-browser");
    expect(data.related.map((r) => r.slug)).toEqual(["whats-on-live-events-guide", "marketplace-credits-season-one"]);
    expect(data.related.every((r) => typeof r.hue === "number")).toBe(true);
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
