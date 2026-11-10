import {
  BLOG_QUERY_MAX,
  type BlogArticle,
  type BlogBlock,
  type BlogRelatedCard,
  type BlogSort,
} from "@dcl/one-ui/apps/sites/blog/blogModel";

export type { BlogBlock, BlogSort };

type BlogCategory = { id: string; title: string; slug: string; url: string };

export type BlogPostCard = {
  id: string;
  slug: string;
  title: string;
  description: string;
  publishedDate: string;
  /** `publishedDate` as an ISO date (YYYY-MM-DD), or "" when it does not parse. */
  publishedAt: string;
  readMinutes: number;
  category: { title: string; slug: string; url: string };
  author: string;
  hue: number;
};

export type BlogPost = {
  id: string;
  slug: string;
  title: string;
  description: string;
  publishedDate: string;
  image: { url: string; width: number; height: number };
  category: BlogCategory;
  author: {
    id: string;
    title: string;
    slug: string;
    image: { url: string };
    url: string;
  };
  hue: number;
  body: BlogBlock[];
};

const DCL_ONE = {
  id: "dcl-one",
  title: "Catalyrst",
  slug: "dcl-one",
  image: { url: "" },
  url: "/blog?author=dcl-one",
};

function cat(slug: string, title: string): BlogCategory {
  return { id: slug, slug, title, url: `/blog?category=${slug}` };
}

const BLOG_POSTS: BlogPost[] = [
  {
    id: "explore-from-your-browser",
    slug: "explore-from-your-browser",
    title: "Explore Decentraland From Your Browser",
    description:
      "The web explorer is live: jump into the world directly from a browser tab \u{2014} no download or install required.",
    publishedDate: "JUL 2, 2026",
    image: { url: "", width: 1200, height: 600 },
    category: cat("platform", "Platform"),
    author: DCL_ONE,
    hue: 268,
    body: [
      {
        type: "p",
        content:
          "You can now walk into Decentraland straight from a browser tab. The web explorer at /play loads the world without any download or install \u{2014} open the page, pick a spot, and you're in.",
      },
      { type: "h3", content: "What you need" },
      {
        type: "ul",
        items: [
          "A modern desktop browser with WebGPU support (recent Chrome works well)",
          "Nothing else \u{2014} no installer, no launcher",
        ],
      },
      {
        type: "p",
        content:
          "The web explorer connects to the same content network as the rest of this site, so the scenes, avatars, and events you see are the real thing. Head to /play and have a look around.",
      },
    ],
  },
  {
    id: "whats-on-live-events-guide",
    slug: "whats-on-live-events-guide",
    title: "What's On: A Live Guide to Events Across the World",
    description:
      "One page that shows what's happening in-world right now and what's coming up, with quick filters for today, this week, and recurring events.",
    publishedDate: "JUL 1, 2026",
    image: { url: "", width: 1200, height: 600 },
    category: cat("platform", "Platform"),
    author: DCL_ONE,
    hue: 200,
    body: [
      {
        type: "p",
        content:
          "Finding something to do in an open world shouldn't take detective work. The What's On page pulls live and upcoming events from the events index into a single view: what's happening right now at the top, and everything coming up grouped by day below.",
      },
      { type: "h3", content: "How to use it" },
      {
        type: "ul",
        items: [
          "Filter by Today, This week, or Recurring to narrow the list",
          "Search events by name",
          "Open any event for details and a link to jump straight to its location in-world",
        ],
      },
      {
        type: "p",
        content:
          "The listing reflects the event data this node serves \u{2014} no editorial curation, just what's actually scheduled. Check it out at /whats-on.",
      },
    ],
  },
  {
    id: "marketplace-credits-season-one",
    slug: "marketplace-credits-season-one",
    title: "Marketplace Credits Season One Is Live",
    description:
      "Complete weekly goals in the Marketplace to earn credits you can spend on wearables and emotes. Season One runs June 29 through August 24, 2026.",
    publishedDate: "JUN 29, 2026",
    image: { url: "", width: 1200, height: 600 },
    category: cat("announcements", "Announcements"),
    author: DCL_ONE,
    hue: 130,
    body: [
      {
        type: "p",
        content:
          "Season One of the Marketplace Credits program starts today. Each week you get a fresh set of goals; complete them, claim your credits, and spend them on wearables and emotes in the Marketplace.",
      },
      { type: "h3", content: "How the season works" },
      {
        type: "ul",
        items: [
          "Season One runs from June 29 to August 24, 2026 \u{2014} eight weekly rounds",
          "Goals reset every week; unclaimed progress doesn't carry over",
          "The season's total credit pool is capped at 100,000 MANA",
        ],
      },
      {
        type: "p",
        content:
          "Your progress, claimable credits, and the weekly countdown are all on the credits page in the Marketplace. Credits earned during the season expire when the season ends, so spend them before August 24.",
      },
    ],
  },
];

const MONTHS = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

/** Reads the "JUL 2, 2026" dates the posts carry as an ISO date; "" when the value does not parse. */
export function blogDateToIso(value: string): string {
  const m = /^([A-Za-z]{3}) (\d{1,2}), (\d{4})$/.exec(value.trim());
  if (!m) return "";
  const month = MONTHS.indexOf(m[1].toUpperCase());
  const day = Number(m[2]);
  if (month < 0 || day < 1 || day > 31) return "";
  return `${m[3]}-${String(month + 1).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}

const WORDS_PER_MINUTE = 200;

/** Reading time of a post's body at 200 words per minute, at least one minute. */
export function blogReadMinutes(body: BlogBlock[]): number {
  const text = body.map((b) => (b.type === "ul" ? b.items.join(" ") : b.content)).join(" ");
  const words = text.split(/\s+/).filter(Boolean).length;
  return Math.max(1, Math.ceil(words / WORDS_PER_MINUTE));
}

function toCard(p: BlogPost): BlogPostCard {
  return {
    id: p.id,
    slug: p.slug,
    title: p.title,
    description: p.description,
    publishedDate: p.publishedDate,
    publishedAt: blogDateToIso(p.publishedDate),
    readMinutes: blogReadMinutes(p.body),
    category: { title: p.category.title, slug: p.category.slug, url: p.category.url },
    author: p.author.title,
    hue: p.hue,
  };
}

export function blogPostCards(categorySlug?: string): BlogPostCard[] {
  return BLOG_POSTS.filter(
    (p) => !categorySlug || p.category.slug === categorySlug,
  ).map(toCard);
}

/** The categories that have at least one post, in the order they first appear. */
export function blogCategoryList(): { slug: string; title: string }[] {
  const seen = new Map<string, string>();
  for (const p of BLOG_POSTS) if (!seen.has(p.category.slug)) seen.set(p.category.slug, p.category.title);
  return [...seen].map(([slug, title]) => ({ slug, title }));
}

/** `?sort=`: "oldest" sorts oldest first; anything else, including no value, is newest first. */
export function parseBlogSort(value: string | null | undefined): BlogSort {
  return value === "oldest" ? "oldest" : "newest";
}

/** `?q=` trimmed and capped; "" means no search. */
export function parseBlogQuery(value: string | null | undefined): string {
  return (value ?? "").trim().slice(0, BLOG_QUERY_MAX);
}

/** Sorts by publication date; posts whose date does not parse go last, in data order. */
export function sortBlogCards(cards: BlogPostCard[], sort: BlogSort): BlogPostCard[] {
  return cards
    .map((card, index) => ({ card, index }))
    .sort((a, b) => {
      const x = a.card.publishedAt;
      const y = b.card.publishedAt;
      if (!x || !y) return x === y ? a.index - b.index : x ? -1 : 1;
      if (x === y) return a.index - b.index;
      return (x < y ? -1 : 1) * (sort === "oldest" ? 1 : -1);
    })
    .map(({ card }) => card);
}

/** The index listing: category filter, then a case-insensitive match of `query` in title, description or category title, sorted. */
export function searchBlogPosts({
  category = "",
  query = "",
  sort = "newest",
}: { category?: string; query?: string; sort?: BlogSort }): BlogPostCard[] {
  const needle = query.trim().toLowerCase();
  const matches = blogPostCards(category).filter(
    (p) =>
      !needle ||
      p.title.toLowerCase().includes(needle) ||
      p.description.toLowerCase().includes(needle) ||
      p.category.title.toLowerCase().includes(needle),
  );
  return sortBlogCards(matches, sort);
}

export function findBlogPost(slug: string): BlogPost | null {
  return BLOG_POSTS.find((p) => p.slug === slug) ?? null;
}

/** A post as the article page shows it. */
export function blogArticle(post: BlogPost): BlogArticle {
  return { ...toCard(post), body: post.body };
}

export function relatedBlogPosts(post: BlogPost, limit = 3): BlogRelatedCard[] {
  return BLOG_POSTS.filter(
    (p) => p.slug !== post.slug && p.category.slug === post.category.slug,
  )
    .concat(BLOG_POSTS.filter((p) => p.slug !== post.slug))
    .filter((p, i, arr) => arr.findIndex((x) => x.slug === p.slug) === i)
    .slice(0, limit)
    .map((p) => ({
      slug: p.slug,
      title: p.title,
      publishedAt: blogDateToIso(p.publishedDate),
      category: { title: p.category.title, slug: p.category.slug },
      hue: p.hue,
    }));
}
