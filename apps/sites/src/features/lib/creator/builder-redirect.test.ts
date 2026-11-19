import { describe, expect, it } from "vitest";

import { REDIRECTS, buildDestination } from "./builder-redirect";

const from = (path: string) => new URL(`https://decentraland.org${path}`);

describe("buildDestination", () => {
  it("should open the collection a publish link names", () => {
    expect(buildDestination(REDIRECTS["publish-collection"], from("/builder/publish-collection?collection=abc"), {})).toBe(
      "/create/collections/abc",
    );
  });

  it("should open the collections list for a publish link without a collection", () => {
    expect(buildDestination(REDIRECTS["publish-collection"], from("/builder/publish-collection"), {})).toBe("/create/collections");
  });

  it("should keep every value of a repeated curation filter", () => {
    expect(buildDestination(REDIRECTS.curation, from("/builder/curation?status=a&status=b&assignee=me"), {})).toBe(
      "/create/curation?status=a&status=b&assignee=me",
    );
  });

  it("should take a collection from the legacy path", () => {
    expect(buildDestination(REDIRECTS["collection-detail"], from("/builder/collection/abc"), { id: "abc" })).toBe(
      "/create/collections/abc",
    );
  });

  it("should open the collections list for an item link, which names no collection", () => {
    expect(buildDestination(REDIRECTS["item-detail"], from("/builder/item/i1"), { id: "i1" })).toBe("/create/collections");
  });
});
