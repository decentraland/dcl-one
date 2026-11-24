import { describe, expect, it } from "vitest";

import { communityPageView } from "./page-view";

describe("community page view", () => {
  it("names a community by its name, with its id, privacy and member count", () => {
    expect(communityPageView({ id: "c1", name: "Builders", privacy: "private", membersCount: 12 })).toEqual({
      name: "Builders",
      properties: { communityId: "c1", privacy: "private", membersCount: 12 },
    });
  });

  it("sends nothing until the community is read", () => {
    expect(communityPageView(undefined)).toBeNull();
  });
});
