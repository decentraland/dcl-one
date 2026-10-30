import { createElement } from "react";
import { describe, expect, it } from "vitest";

import { blogPostCards, type BlogPost } from "@/core/lib/content/blog";
import type { Place } from "../catalyst/schema";
import {
  blogIndexToMarkdown,
  blogPostToMarkdown,
  communityToMarkdown,
  discoverToMarkdown,
  estimateTokens,
  eventDetailToMarkdown,
  legalDocToMarkdown,
  markdownResponse,
  placeToMarkdown,
  placesIndexToMarkdown,
  profileToMarkdown,
  shopPageToMarkdown,
  reactNodeToMarkdown,
  wantsMarkdown,
  whatsOnToMarkdown,
  type AgentLegalDoc,
} from "./markdown";

const NO_HTML = /<\/?[a-z][^>]*>/i;

function req(url: string, headers: Record<string, string> = {}): Request {
  return new Request(url, { headers });
}

describe("wantsMarkdown", () => {
  it("is true for an Accept carrying text/markdown, ?format=md (any case) or a trailing .md path; false otherwise", () => {
    expect(wantsMarkdown(req("https://x.io/blog", { accept: "text/markdown" }))).toBe(true);
    expect(
      wantsMarkdown(req("https://x.io/blog", { accept: "text/html, text/markdown;q=0.9" })),
    ).toBe(true);
    expect(wantsMarkdown(req("https://x.io/blog?format=md"))).toBe(true);
    expect(wantsMarkdown(req("https://x.io/blog?format=MD&x=1"))).toBe(true);
    expect(wantsMarkdown(req("https://x.io/blog/post.md"))).toBe(true);
    expect(
      wantsMarkdown(
        req("https://x.io/blog", {
          accept: "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
        }),
      ),
    ).toBe(false);
    expect(wantsMarkdown(req("https://x.io/blog", { accept: "application/json" }))).toBe(false);
    expect(wantsMarkdown(req("https://x.io/blog"))).toBe(false);
    expect(wantsMarkdown(req("https://x.io/blog?format=html"))).toBe(false);
  });
});

describe("markdownResponse", () => {
  it("sets content-type, token count and Vary headers, honoring status and cacheControl", async () => {
    expect(estimateTokens("")).toBe(0);
    const md = "# Hello\n\nWorld";
    const res = markdownResponse(md);
    expect(res.status).toBe(200);
    expect(res.headers.get("Content-Type")).toBe("text/markdown; charset=utf-8");
    expect(res.headers.get("x-markdown-tokens")).toBe(String(estimateTokens(md)));
    expect(res.headers.get("Vary")).toBe("Accept");
    expect(res.headers.get("Cache-Control")).toBeNull();
    expect(await res.text()).toBe(md);
    const custom = markdownResponse("# x", { status: 404, cacheControl: "public, max-age=60" });
    expect(custom.status).toBe(404);
    expect(custom.headers.get("Cache-Control")).toBe("public, max-age=60");
  });
});

describe("reactNodeToMarkdown", () => {
  it("passes text through, drops nullish and booleans, links anchors, bolds strong and flattens children", () => {
    expect(reactNodeToMarkdown("hello")).toBe("hello");
    expect(reactNodeToMarkdown(42)).toBe("42");
    expect(reactNodeToMarkdown(null)).toBe("");
    expect(reactNodeToMarkdown(undefined)).toBe("");
    expect(reactNodeToMarkdown(false)).toBe("");
    expect(reactNodeToMarkdown(createElement("a", { href: "https://x.io" }, "link"))).toBe(
      "[link](https://x.io)",
    );
    expect(reactNodeToMarkdown(createElement("strong", null, "bold"))).toBe("**bold**");
    expect(
      reactNodeToMarkdown(["Visit ", createElement("a", { href: "u" }, "here"), "."]),
    ).toBe("Visit [here](u).");
  });
});

describe("blogPostToMarkdown", () => {
  it("leads with the H1, includes description and metadata, renders body blocks; no HTML", () => {
    const post: BlogPost = {
      id: "test-post",
      slug: "test-post",
      title: "Test Post Title",
      description: "A test description of the post.",
      publishedDate: "JAN 1, 2026",
      image: { url: "", width: 1200, height: 600 },
      category: {
        id: "announcements",
        slug: "announcements",
        title: "Announcements",
        url: "/blog?category=announcements",
      },
      author: {
        id: "test-author",
        title: "Test Author",
        slug: "test-author",
        image: { url: "" },
        url: "/blog?author=test-author",
      },
      hue: 0,
      body: [
        { type: "p", content: "An opening paragraph." },
        { type: "h2", content: "A Brand-New Desktop Client" },
        { type: "quote", content: "A pull quote." },
        { type: "h3", content: "What's New for Explorers" },
        { type: "ul", items: ["Photorealistic visuals with upgraded lighting and shadows"] },
      ],
    };
    const md = blogPostToMarkdown(post);
    expect(md.startsWith(`# ${post.title}`)).toBe(true);
    expect(md).toContain(post.description);
    expect(md).toContain(post.category.title);
    expect(md).toContain(post.publishedDate);
    expect(md).toContain(post.author.title);
    expect(md).toContain("## A Brand-New Desktop Client");
    expect(md).toContain("### What's New for Explorers");
    expect(md).toMatch(/^> /m);
    expect(md).toContain("- Photorealistic visuals with upgraded lighting and shadows");
    expect(md).not.toMatch(NO_HTML);
  });
});

describe("blogIndexToMarkdown", () => {
  it("has the index H1, a count line and an H2 + read-more link per post; no HTML", () => {
    const cards = blogPostCards();
    const md = blogIndexToMarkdown(cards);
    expect(md.startsWith("# Decentraland Blog")).toBe(true);
    expect(md).toContain(`${cards.length} posts.`);
    expect(md).toContain(`## ${cards[0].title}`);
    expect(md).toContain(`[Read more](/blog/${cards[0].slug})`);
    expect(md).not.toMatch(NO_HTML);
  });
});

describe("legalDocToMarkdown", () => {
  it("leads with the H1 and intro, renders sections, subheadings, lists and embedded anchors; no HTML", () => {
    const doc: AgentLegalDoc = {
      title: "Terms of Use",
      intro: "Please read carefully.",
      sections: [
        {
          id: "acceptance",
          heading: "1. Acceptance of Terms",
          body: [
            "By using the service you agree to these terms.",
            { type: "h3", id: "scope", content: "1.1 Scope" },
            {
              type: "p",
              content: ["For more info visit ", createElement("a", { href: "https://dao.decentraland.org" }, "the DAO"), "."],
            },
            { type: "ul", items: ["First point", "Second point"] },
          ],
        },
      ],
    };
    const md = legalDocToMarkdown(doc);
    expect(md.startsWith("# Terms of Use")).toBe(true);
    expect(md).toContain("Please read carefully.");
    expect(md).toContain("## 1. Acceptance of Terms");
    expect(md).toContain("### 1.1 Scope");
    expect(md).toContain("- First point");
    expect(md).toContain("[the DAO](https://dao.decentraland.org)");
    expect(md).not.toMatch(NO_HTML);
  });
});

function makePlace(overrides: Partial<Place>): Place {
  return {
    id: "",
    title: null,
    description: null,
    image: null,
    owner: null,
    positions: [],
    base_position: "0,0",
    updated_at: null,
    created_at: null,
    contact_name: null,
    categories: [],
    highlighted: false,
    highlighted_image: null,
    user_count: null,
    user_visits: 0,
    favorites: 0,
    likes: 0,
    like_rate: null,
    world: false,
    world_name: null,
    ...overrides,
  };
}

describe("placeToMarkdown", () => {
  it("leads with the H1 and description and lists key metadata as bullets; no HTML", () => {
    const md = placeToMarkdown(
      makePlace({
        id: "abc-123",
        title: "Neon Arcade",
        description: "A retro arcade in the heart of the city.",
        base_position: "10,20",
        positions: ["10,20", "11,20"],
        contact_name: "Pixel Studio",
        categories: ["game", "social"],
        user_count: 42,
        likes: 100,
        favorites: 7,
        like_rate: 0.83,
        world: false,
      }),
    );
    expect(md.startsWith("# Neon Arcade")).toBe(true);
    expect(md).toContain("A retro arcade in the heart of the city.");
    expect(md).toContain("- Coordinates: 10,20");
    expect(md).toContain("- Parcels: 2");
    expect(md).toContain("- Creator: Pixel Studio");
    expect(md).toContain("- Visitors: 42");
    expect(md).toContain("- Categories: game, social");
    expect(md).toContain("- Approval: 83%");
    expect(md).not.toMatch(NO_HTML);
  });
});

describe("placesIndexToMarkdown", () => {
  it("has the index H1, a singular count line and an H2 + view link per place; no HTML", () => {
    const md = placesIndexToMarkdown([
      makePlace({
        id: "abc-123",
        title: "Neon Arcade",
        description: "A retro arcade.",
        base_position: "10,20",
        user_count: 42,
        likes: 100,
        world: false,
      }),
    ]);
    expect(md.startsWith("# Decentraland Places")).toBe(true);
    expect(md).toContain("1 place.");
    expect(md).toContain("## Neon Arcade");
    expect(md).toContain("[View](/places/abc-123)");
    expect(md).not.toMatch(NO_HTML);
  });
});

describe("discoverToMarkdown", () => {
  it("renders hero + events/hotspots/rituals rails; no HTML", () => {
    const content = {
      hero: { kicker: "", title: "Discover Decentraland", subtitle: "What's happening now",
        downloads: null, downloadsLabel: "", cta: { desktop: {}, epic: {} }, platforms: [] },
      rails: [
        { id: "e", kind: "events", title: "Events", viewAll: null,
          items: [{ id: "e1", category: "Music", title: "DJ Set", when: "Tonight", live: true, image: null }] },
        { id: "h", kind: "hotspots", title: "Hotspots", viewAll: null,
          items: [{ id: "h1", title: "Central Plaza", online: 42, image: null, href: "/places/x" }] },
        { id: "r", kind: "rituals", title: "Rituals", viewAll: null,
          items: [{ id: "r1", title: "Friday Trivia", day: "Fridays", image: null, href: "/places/y" }] },
      ],
      comeHangOut: { title: "", downloads: null, downloadsLabel: "" },
    } as unknown as Parameters<typeof discoverToMarkdown>[0];
    const md = discoverToMarkdown(content);
    expect(md.startsWith("# Discover Decentraland")).toBe(true);
    expect(md).toContain("What's happening now");
    expect(md).toContain("## Events");
    expect(md).toContain("DJ Set");
    expect(md).toContain("Music");
    expect(md).toContain("LIVE");
    expect(md).toContain("## Hotspots");
    expect(md).toContain("[Central Plaza](/places/x)");
    expect(md).toContain("42 online");
    expect(md).toContain("## Rituals");
    expect(md).toContain("[Friday Trivia](/places/y)");
    expect(md).not.toMatch(NO_HTML);
  });
});

describe("shopPageToMarkdown", () => {
  it("renders a Shop item page from its head and public facts, never a price", () => {
    const md = shopPageToMarkdown({
      page: "itemDetail",
      seo: { title: "Pirate Hat", description: "A hat for pirates.", canonical: "https://example.org/shop/item/0xabc/7", noindex: false },
      queries: [
        [["item-meta", "0xabc", "7"], { name: "Pirate Hat", rarity: "rare", category: "wearable", creator: "0x1", network: "MATIC", available: 3, isSmart: false, utility: null }, 1],
        [["collection-meta", "0xabc"], { name: "Sea Things", contractAddress: "0xabc", creator: "0x1" }, 1],
        [["shop-item", "0xabc", "7"], { priceCredits: 12, manaWei: "1000000000000000000" }, 1],
      ],
    });
    expect(md).toContain("# Pirate Hat");
    expect(md).toContain("A hat for pirates.");
    expect(md).toContain("- Collection: Sea Things (0xabc)");
    expect(md).toContain("- Rarity: rare");
    expect(md).toContain("- Available to mint: 3");
    expect(md).toContain("Shop page: https://example.org/shop/item/0xabc/7");
    expect(md).not.toMatch(/credit|MANA|12/);
  });
});

describe("communityToMarkdown", () => {
  it("renders community info, members, and events; no HTML", () => {
    const detail = {
      source: "live",
      community: {
        id: "c1", name: "Builders Guild", description: "Where scene builders meet.",
        ownerAddress: "0xowner", ownerName: "chief", privacy: "public", membersCount: 128, role: "none",
      },
      members: [
        { memberAddress: "0xm1", name: "ada", role: "owner", hasClaimedName: true, profilePictureUrl: "" },
        { memberAddress: "0xm2", name: "", role: "member", hasClaimedName: false, profilePictureUrl: "" },
      ],
      events: [{ id: "ev1", name: "Build Jam", image: "", creatorName: "ada", timeLabel: "Sat 6pm" }],
    } as unknown as Parameters<typeof communityToMarkdown>[0];
    const md = communityToMarkdown(detail);
    expect(md.startsWith("# Builders Guild")).toBe(true);
    expect(md).toContain("Where scene builders meet.");
    expect(md).toContain("- Owner: chief");
    expect(md).toContain("- Privacy: public");
    expect(md).toContain("- Members: 128");
    expect(md).toContain("## Members");
    expect(md).toContain("- ada (owner)");
    expect(md).toContain("- 0xm2 (member)");
    expect(md).toContain("## Events");
    expect(md).toContain("Build Jam");
    expect(md).not.toMatch(NO_HTML);
  });
});

describe("profileToMarkdown", () => {
  it("renders name, bio, facts, info fields and links; no HTML", () => {
    const p = {
      address: "0xabc0000000000000000000000000000000000001",
      name: "pixelmancer", hasClaimedName: true, nameColor: "#fff", mutualCount: 3,
      bio: "Builder of odd little scenes.", accountUrl: "/account",
      info: [
        { key: "country", label: "Country", value: "Argentina", icon: "globe" },
        { key: "language", label: "Language", value: "", icon: "translate" },
      ],
      links: [{ title: "Portfolio", url: "https://example.com" }],
      equipped: [],
    } as unknown as Parameters<typeof profileToMarkdown>[0];
    const md = profileToMarkdown(p);
    expect(md.startsWith("# pixelmancer")).toBe(true);
    expect(md).toContain("Builder of odd little scenes.");
    expect(md).toContain("- Claimed name: yes");
    expect(md).toContain("- Mutual friends: 3");
    expect(md).toContain("## Info");
    expect(md).toContain("- Country: Argentina");
    expect(md).not.toContain("Language:");
    expect(md).toContain("## Links");
    expect(md).toContain("[Portfolio](https://example.com)");
    expect(md).not.toMatch(NO_HTML);
  });

  it("renders an explicit empty state for an address with no public profile", () => {
    const p = {
      address: "0xdef", name: "0xdef", hasClaimedName: false, nameColor: "#fff",
      mutualCount: 0, bio: "", accountUrl: "", info: [], links: [], equipped: [],
    } as unknown as Parameters<typeof profileToMarkdown>[0];
    const md = profileToMarkdown(p);
    expect(md.startsWith("# Decentraland Profile")).toBe(true);
    expect(md).toMatch(/no public .*profile/i);
    expect(md).toContain("- Address: 0xdef");
    expect(md).toContain("- Claimed name: no");
    expect(md).not.toContain("## Info");
  });
});

describe("whatsOnToMarkdown", () => {
  it("renders live + upcoming events with links, coords, counts, filter note; no HTML", () => {
    const live = [
      { id: "e1", name: "DJ Night", live: true, x: 10, y: -4, user_name: "dj", total_attendees: 42 },
    ] as unknown as Parameters<typeof whatsOnToMarkdown>[0]["live"];
    const upcoming = [
      {
        id: "e2", name: "Art Expo", start_at: "2026-07-10T18:00:00Z",
        x: 0, y: 0, scene_name: "Gallery", recurrent: true, recurrent_frequency: "weekly",
      },
    ] as unknown as Parameters<typeof whatsOnToMarkdown>[0]["upcoming"];
    const md = whatsOnToMarkdown({ live, upcoming, filter: "week" });
    expect(md.startsWith("# What's On in Decentraland")).toBe(true);
    expect(md).toContain("1 live now, 1 upcoming.");
    expect(md).toContain("(filter: week)");
    expect(md).toContain("## Live now");
    expect(md).toContain("[DJ Night](/whats-on/e1)");
    expect(md).toContain("LIVE");
    expect(md).toContain("42 attending");
    expect(md).toContain("## Upcoming");
    expect(md).toContain("[Art Expo](/whats-on/e2)");
    expect(md).toContain("(0,0)");
    expect(md).toContain("weekly");
    expect(md).not.toMatch(NO_HTML);
  });

  it("shows an empty state when there are no events", () => {
    const md = whatsOnToMarkdown({ live: [], upcoming: [] });
    expect(md).toContain("0 live now, 0 upcoming.");
    expect(md).toContain("No events right now");
  });
});

describe("eventDetailToMarkdown", () => {
  it("renders title, description, metadata bullets, jump link; no HTML", () => {
    const e = {
      id: "e2", name: "Art Expo", description: "A curated show.",
      start_at: "2026-07-10T18:00:00Z", x: 12, y: 34, user_name: "curator",
      total_attendees: 7, recurrent: true, recurrent_frequency: "weekly",
      url: "https://decentraland.org/jump/x",
    } as unknown as Parameters<typeof eventDetailToMarkdown>[0];
    const md = eventDetailToMarkdown(e);
    expect(md.startsWith("# Art Expo")).toBe(true);
    expect(md).toContain("A curated show.");
    expect(md).toContain("- Host: curator");
    expect(md).toContain("- Location: (12,34)");
    expect(md).toContain("- Recurring: weekly");
    expect(md).toContain("- Attendees: 7");
    expect(md).toContain("[Jump in](https://decentraland.org/jump/x)");
    expect(md).not.toMatch(NO_HTML);
  });

  it("builds a jump URL from coordinates when url is absent", () => {
    const e = {
      id: "e3", name: "Meetup", x: 5, y: 6,
    } as unknown as Parameters<typeof eventDetailToMarkdown>[0];
    const md = eventDetailToMarkdown(e);
    expect(md).toContain("](/play/?position=");
    expect(md).toContain("5%2C6");
  });
});

