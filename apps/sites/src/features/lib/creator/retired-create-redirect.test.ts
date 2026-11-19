import { describe, expect, it } from "vitest";

import { retiredCreateDestination } from "./retired-create-redirect";

const to = (path: string) => retiredCreateDestination(new URL(`https://decentraland.org${path}`));

describe("retiredCreateDestination", () => {
  it.each([
    ["/create/wearables", "/create/collections"],
    ["/create/wearables/collections/new", "/create/collections"],
    ["/create/wearables/collections/abc", "/create/collections/abc"],
    ["/create/wearables/items/i1", "/create/collections/editor?item=i1"],
    ["/create/wearables/item-editor?collection=c1&item=i1&step=2", "/create/collections/editor?collection=c1&item=i1"],
    ["/create/wearables/publish?collection=c1", "/create/collections/c1"],
    ["/create/curate?status=a&status=b&step=3", "/create/curation?status=a&status=b"],
  ])("should send %s to %s", (from, destination) => {
    expect(to(from)).toBe(destination);
  });
});
