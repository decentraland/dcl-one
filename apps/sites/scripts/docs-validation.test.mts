import { describe, expect, it } from "vitest";
import { validateDocs } from "./docs-validation.mts";

describe("generated docs validation", () => {
  it("checks page and decoded asset targets while ignoring code and comments", () => {
    const pages = new Map([["/docs/", '[page](/docs/page/) ![image](/docs/a%20b.webp) `![x](/docs/no.webp)` <!-- [x](/docs/no/) -->'], ["/docs/page/", "ok"]]);
    expect(validateDocs(pages, new Set(["a b.webp"]))).toEqual([]);
    expect(validateDocs(pages, new Set())).toEqual(["broken local reference: /docs/ /docs/a%20b.webp"]);
  });
  it("reports missing pages, media, and control characters", () => {
    expect(validateDocs(new Map([["/docs/", '[x](/docs/missing/) <img src="/docs/no.png">\x01']]), new Set())).toEqual([
      "control character: /docs/", "broken local reference: /docs/ /docs/missing/", "broken local reference: /docs/ /docs/no.png",
    ]);
  });
});
