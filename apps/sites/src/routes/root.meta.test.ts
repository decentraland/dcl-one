import { describe, expect, it } from "vitest";
import { meta } from "./root";
import { canonicalHrefs, rootMatches, tagContent } from "./lib/social-meta.test-utils";

type Args = Parameters<typeof meta>[0];
const args = (extra: Partial<Args>) => ({ matches: rootMatches("https://dcl.test"), ...extra }) as unknown as Args;

describe("root meta", () => {
  it("gives a page without a head the shell card but no canonical or og:url", () => {
    const tags = meta(args({ error: undefined }));
    expect(tags[0]).toEqual({ title: "Decentraland" });
    expect(tagContent(tags, "og:title")).toEqual(["Decentraland — Come Hang Out"]);
    expect(tagContent(tags, "og:url")).toEqual([]);
    expect(canonicalHrefs(tags)).toEqual([]);
  });

  it("marks an unknown path noindex with no card", () => {
    const tags = meta(args({ error: { status: 404, statusText: "Not Found", internal: false, data: null } }));
    expect(tagContent(tags, "robots")).toEqual(["noindex"]);
    expect(tagContent(tags, "og:title")).toEqual([]);
  });

  it("keeps the shell card for a server error", () => {
    const tags = meta(args({ error: new Error("boom") }));
    expect(tagContent(tags, "og:title")).toEqual(["Decentraland — Come Hang Out"]);
  });
});
