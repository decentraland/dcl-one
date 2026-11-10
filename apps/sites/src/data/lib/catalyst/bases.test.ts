import { describe, expect, it } from "vitest";

import { blogCmsUrl, playUrl, publicPageUrl, siteUrl } from "./bases";

describe("siteUrl + playUrl (server side: no window)", () => {
  it("links stay on this origin: relative without PUBLIC_CATALYST_URL, absolute under it", () => {
    expect(siteUrl("/shop", {})).toBe("/shop");
    expect(siteUrl("shop", { PUBLIC_CATALYST_URL: "https://catalyst.example/" })).toBe("https://catalyst.example/shop");
    expect(playUrl(undefined, {})).toBe("/play/");
    expect(playUrl({ position: "-3,-2" }, {})).toBe("/play/?position=-3%2C-2");
    expect(playUrl({ realm: "a b.dcl.eth", position: undefined }, {})).toBe("/play/?realm=a+b.dcl.eth");
    expect(playUrl({ position: "1,2" }, { PUBLIC_CATALYST_URL: "https://catalyst.example" })).toBe(
      "https://catalyst.example/play/?position=1%2C2",
    );
  });

  it("BEVY_PLAY_URL moves the entry: a path on this origin or an absolute URL", () => {
    expect(playUrl({ realm: "x.dcl.eth" }, { BEVY_PLAY_URL: "/explorer/" })).toBe("/explorer/?realm=x.dcl.eth");
    expect(playUrl({ realm: "x.dcl.eth" }, { BEVY_PLAY_URL: "https://play.example/app" })).toBe(
      "https://play.example/app/?realm=x.dcl.eth",
    );
    const params = new URLSearchParams({ position: "0,0", realm: "dcl-one" });
    expect(playUrl(params, { PUBLIC_CATALYST_URL: "https://c.example", BEVY_PLAY_URL: "/play" })).toBe(
      "https://c.example/play/?position=0%2C0&realm=dcl-one",
    );
  });
});

describe("publicPageUrl", () => {
  const request = new Request("http://10.0.0.5:3000/blog/post?x=1");

  it("names the configured public origin, trailing slash or not", () => {
    expect(publicPageUrl(request, "/blog/post", { PUBLIC_CATALYST_URL: "https://catalyst.example/" })).toBe(
      "https://catalyst.example/blog/post",
    );
    expect(publicPageUrl(request, "/blog/post", { PUBLIC_CATALYST_URL: " https://catalyst.example " })).toBe(
      "https://catalyst.example/blog/post",
    );
  });

  it("falls back to the request's origin when unset or blank", () => {
    expect(publicPageUrl(request, "/blog/post", {})).toBe("http://10.0.0.5:3000/blog/post");
    expect(publicPageUrl(request, "/blog/post", { PUBLIC_CATALYST_URL: "   " })).toBe("http://10.0.0.5:3000/blog/post");
  });
});

describe("blogCmsUrl", () => {
  it("reads BLOG_CMS_URL trimmed and without a trailing slash", () => {
    expect(blogCmsUrl({ BLOG_CMS_URL: " https://cms.example/spaces/s/environments/e/ " })).toBe(
      "https://cms.example/spaces/s/environments/e",
    );
  });

  it("is undefined when BLOG_CMS_URL is unset or blank", () => {
    expect(blogCmsUrl({})).toBeUndefined();
    expect(blogCmsUrl({ BLOG_CMS_URL: "   " })).toBeUndefined();
  });
});
