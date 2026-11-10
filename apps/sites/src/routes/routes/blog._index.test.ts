import { describe, expect, it } from "vitest";
import { loader, meta } from "./blog._index";

type Data = {
  posts: { slug: string; category: { slug: string } }[];
  postCount: number;
  category: string;
  query: string;
  sort: string;
  categories: { slug: string; title: string }[];
};

async function load(search = "") {
  const result = (await loader({ request: new Request(`https://sites.test/blog${search}`) } as never)) as unknown as {
    data: Data;
    init: ResponseInit | null;
  };
  return result;
}

const slugs = (d: Data) => d.posts.map((p) => p.slug);

describe("blog index loader", () => {
  it("lists every post newest first, with the categories that have posts", async () => {
    const { data } = await load();
    expect(slugs(data)).toEqual(["explore-from-your-browser", "whats-on-live-events-guide", "marketplace-credits-season-one"]);
    expect(data.categories).toEqual([
      { slug: "platform", title: "Platform" },
      { slug: "announcements", title: "Announcements" },
    ]);
    expect(data).toMatchObject({ category: "", query: "", sort: "newest" });
  });

  it("filters by category", async () => {
    const { data } = await load("?category=platform");
    expect(data.posts.every((p) => p.category.slug === "platform")).toBe(true);
    expect(data.posts).toHaveLength(2);
  });

  it("returns no posts for an unknown category", async () => {
    const { data } = await load("?category=nope");
    expect(data.posts).toEqual([]);
    expect(data.category).toBe("nope");
  });

  it("matches the query in title or description, case-insensitively", async () => {
    expect(slugs((await load("?q=BROWSER")).data)).toEqual(["explore-from-your-browser"]);
    expect(slugs((await load("?q=wearables")).data)).toEqual(["marketplace-credits-season-one"]);
  });

  it("matches the category title too, as the index always did", async () => {
    const { data } = await load("?q=platform");
    expect(slugs(data)).toEqual(["explore-from-your-browser", "whats-on-live-events-guide"]);
    expect(data.posts.every((p) => p.category.slug === "platform")).toBe(true);
    expect(slugs((await load("?q=ANNOUNCE")).data)).toEqual(["marketplace-credits-season-one"]);
  });

  it("counts the category's posts for telemetry, whatever the search", async () => {
    expect((await load("?q=zzzz")).data.postCount).toBe(3);
    expect((await load("?category=platform&q=zzzz")).data.postCount).toBe(2);
    expect((await load("?category=nope")).data.postCount).toBe(0);
  });

  it("treats an empty or whitespace query as no search", async () => {
    for (const q of ["?q=", "?q=%20%20%20"]) {
      const { data } = await load(q);
      expect(data.query).toBe("");
      expect(data.posts).toHaveLength(3);
    }
  });

  it("returns no posts for a query with no match", async () => {
    const { data } = await load("?q=xyz");
    expect(data.posts).toEqual([]);
    expect(data.query).toBe("xyz");
  });

  it("caps a very long query", async () => {
    const { data } = await load(`?q=${"a".repeat(500)}`);
    expect(data.query).toHaveLength(200);
    expect(data.posts).toEqual([]);
  });

  it("sorts oldest first on request and falls back to newest for any other value", async () => {
    expect(slugs((await load("?sort=oldest")).data)).toEqual([
      "marketplace-credits-season-one",
      "whats-on-live-events-guide",
      "explore-from-your-browser",
    ]);
    const bad = (await load("?sort=sideways")).data;
    expect(bad.sort).toBe("newest");
    expect(slugs(bad)[0]).toBe("explore-from-your-browser");
  });

  it("combines category, query and sort", async () => {
    const { data } = await load("?category=platform&q=the&sort=oldest");
    expect(slugs(data)).toEqual(["whats-on-live-events-guide", "explore-from-your-browser"]);
  });

  it("keeps its title and description", () => {
    expect(meta()).toEqual([
      { title: "Decentraland Blog" },
      { name: "description", content: "News, creator stories, events, and updates from the Decentraland community." },
    ]);
  });
});
