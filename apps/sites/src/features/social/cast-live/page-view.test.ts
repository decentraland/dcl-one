import { describe, expect, it } from "vitest";

import { CAST_STREAMER_TITLE, CAST_WATCHER_TITLE, castPageView } from "./page-view";

describe("cast page views", () => {
  it("names the streamer page by its base title until the stream's place is known", () => {
    expect(castPageView(CAST_STREAMER_TITLE, null)).toEqual({ name: "Streaming on Decentraland Cast" });
  });

  it("names the streamer page after the place, with its name, location and kind", () => {
    expect(castPageView(CAST_STREAMER_TITLE, { placeName: "Genesis Plaza", location: "0,0", isWorld: false })).toEqual({
      name: "Genesis Plaza — Streaming on Decentraland Cast",
      properties: { placeName: "Genesis Plaza", location: "0,0", isWorld: false },
    });
  });

  it("names the watcher page after a world", () => {
    expect(castPageView(CAST_WATCHER_TITLE, { placeName: "myworld.dcl.eth", location: "myworld.dcl.eth", isWorld: true })).toEqual({
      name: "myworld.dcl.eth — Watching on Decentraland Cast",
      properties: { placeName: "myworld.dcl.eth", location: "myworld.dcl.eth", isWorld: true },
    });
  });

  it("keeps the base title when the place has no name", () => {
    expect(castPageView(CAST_WATCHER_TITLE, { placeName: "", location: "10,10", isWorld: false })?.name).toBe("Watching on Decentraland Cast");
  });
});
