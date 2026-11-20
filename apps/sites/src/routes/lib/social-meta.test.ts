import { describe, expect, it } from "vitest";
import { DEFAULT_SOCIAL_IMAGE, absoluteUrl, homeMeta, notFoundMeta, oneLine, publicBase, socialMeta } from "./social-meta";
import { tagContent as content } from "./social-meta.test-utils";

describe("socialMeta", () => {
  it("writes the full card with one og:type and an absolute canonical", () => {
    const tags = socialMeta({ title: "Genesis Plaza | Decentraland", description: "Hi", image: "https://img.test/a.png", url: "https://dcl.test/places/place/0,0" });
    expect(tags[0]).toEqual({ title: "Genesis Plaza | Decentraland" });
    expect(content(tags, "og:title")).toEqual(["Genesis Plaza | Decentraland"]);
    expect(content(tags, "twitter:title")).toEqual(["Genesis Plaza | Decentraland"]);
    expect(content(tags, "og:description")).toEqual(["Hi"]);
    expect(content(tags, "twitter:description")).toEqual(["Hi"]);
    expect(content(tags, "og:image")).toEqual(["https://img.test/a.png"]);
    expect(content(tags, "twitter:card")).toEqual(["summary_large_image"]);
    expect(content(tags, "og:type")).toEqual(["website"]);
    expect(content(tags, "og:site_name")).toEqual(["Decentraland"]);
    expect(content(tags, "twitter:site")).toEqual(["@decentraland"]);
    expect(content(tags, "og:url")).toEqual(["https://dcl.test/places/place/0,0"]);
    expect(tags).toContainEqual({ tagName: "link", rel: "canonical", href: "https://dcl.test/places/place/0,0" });
  });

  it("falls back to the shared image, and a small image without one stays large", () => {
    const tags = socialMeta({ title: "T", smallImage: true });
    expect(content(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
    expect(content(tags, "twitter:card")).toEqual(["summary_large_image"]);
    expect(content(socialMeta({ title: "T", image: "https://img.test/face.png", smallImage: true }), "twitter:card")).toEqual(["summary"]);
  });

  it("omits blank descriptions and drops the canonical of a noindex page", () => {
    const tags = socialMeta({ title: "T", description: "   ", url: "https://dcl.test/x", noindex: true });
    expect(content(tags, "description")).toEqual([]);
    expect(content(tags, "og:description")).toEqual([]);
    expect(content(tags, "og:url")).toEqual([]);
    expect(tags.some((t) => "rel" in t)).toBe(false);
    expect(content(tags, "robots")).toEqual(["noindex"]);
  });

  it("emits each structured-data object and the extra tags last", () => {
    const tags = socialMeta({ title: "T", jsonLd: [{ "@type": "Event" }], extra: [{ property: "article:section", content: "News" }] });
    expect(tags).toContainEqual({ "script:ld+json": { "@type": "Event" } });
    expect(tags.at(-1)).toEqual({ property: "article:section", content: "News" });
  });
});

describe("homeMeta and notFoundMeta", () => {
  it("keeps the shell's document title apart from its card title", () => {
    const tags = homeMeta("https://dcl.test");
    expect(tags[0]).toEqual({ title: "Decentraland" });
    expect(content(tags, "og:title")).toEqual(["Decentraland — Come Hang Out"]);
    expect(content(tags, "og:url")).toEqual(["https://dcl.test/"]);
    expect(content(tags, "og:logo")).toHaveLength(1);
  });

  it("has no canonical without a public base", () => {
    expect(homeMeta(undefined).some((t) => "rel" in t)).toBe(false);
  });

  it("marks a missing page noindex with no card", () => {
    const tags = notFoundMeta();
    expect(content(tags, "robots")).toEqual(["noindex"]);
    expect(content(tags, "og:title")).toEqual([]);
  });
});

describe("helpers", () => {
  it("reads the root's public base and joins paths without a double slash", () => {
    expect(publicBase([{ id: "root", loaderData: { publicPageBase: "https://dcl.test/" } }])).toBe("https://dcl.test/");
    expect(publicBase([{ id: "root", loaderData: undefined }, undefined])).toBeUndefined();
    expect(absoluteUrl("https://dcl.test/", "/blog")).toBe("https://dcl.test/blog");
    expect(absoluteUrl(undefined, "/blog")).toBeUndefined();
  });

  it("collapses whitespace and treats an empty text as missing", () => {
    expect(oneLine("  a\n\n b\t c ")).toBe("a b c");
    expect(oneLine(" \n ")).toBeUndefined();
    expect(oneLine(null)).toBeUndefined();
  });
});
