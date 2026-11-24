import { describe, expect, it } from "vitest";

import { PLACES_PAGE_VIEW, placePageView } from "./page-view";

describe("places page views", () => {
  it("names the Places page by its English title, with no properties", () => {
    expect(PLACES_PAGE_VIEW).toEqual({ name: "Places — Decentraland" });
  });

  it("names a place by its title, with kind 'place' and no world", () => {
    expect(placePageView({ id: "p1", title: "Genesis Plaza", world: false, world_name: null })).toEqual({
      name: "Genesis Plaza",
      properties: { kind: "place", place_id: "p1", world: null },
    });
  });

  it("names a world by its title, with kind 'world' and the world's name", () => {
    expect(placePageView({ id: "w1", title: "My World", world: true, world_name: "myworld.dcl.eth" })).toEqual({
      name: "My World",
      properties: { kind: "world", place_id: "w1", world: "myworld.dcl.eth" },
    });
  });

  it("sends nothing until the place is read, or when it has no title", () => {
    expect(placePageView(null)).toBeNull();
    expect(placePageView(undefined)).toBeNull();
    expect(placePageView({ id: "p2", title: null, world: false, world_name: null })).toBeNull();
  });
});
