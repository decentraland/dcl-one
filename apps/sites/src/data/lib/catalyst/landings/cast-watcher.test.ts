import { beforeEach, describe, expect, it, vi } from "vitest";

const postJSON = vi.hoisted(() => vi.fn());
vi.mock("../client", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../client")>()),
  postJSON,
}));

import type { AuthIdentity } from "../../auth/types";
import { CatalystError } from "../client";
import {
  WATCHER_TOKEN_PATH,
  requestWatcherCredentials,
  watchErrorFor,
} from "./cast-watcher";
import { resolveWatch } from "./cast-watcher.server";

const url = "https://catalyst.example" + WATCHER_TOKEN_PATH;
const viewer = { address: "0x0000000000000000000000000000000000000001" } as unknown as AuthIdentity;

describe("cast watcher", () => {
  beforeEach(() => postJSON.mockReset());

  it("asks for the token with the viewer's signed POST of the location and display name", async () => {
    postJSON.mockResolvedValue({
      url: "wss://livekit.example",
      token: "example-token",
      roomId: "scene-room",
      identity: "watch:scene-room:1",
      placeName: "Genesis Plaza",
    });
    const credentials = await requestWatcherCredentials("0,0", "Viewer", viewer);
    expect(postJSON).toHaveBeenCalledWith(
      "/comms-gatekeeper/cast/watcher-token",
      { location: "0,0", identity: "Viewer" },
      expect.objectContaining({ identity: viewer }),
    );
    expect(credentials.roomId).toBe("scene-room");
  });

  it("names what a refused join means", () => {
    const refusal = (status: number, message: string) => new CatalystError(message, url, status, true);
    expect(watchErrorFor(refusal(404, "No active stream found for 0,0"))).toBe("no_active_stream");
    expect(watchErrorFor(refusal(401, "Stream access has expired. Please generate a new stream link.")))
      .toBe("access_expired");
    expect(watchErrorFor(refusal(401, "Invalid signature"))).toBe("refused");
    expect(watchErrorFor(refusal(403, "You are banned from this scene"))).toBe("banned");
    expect(watchErrorFor(refusal(503, "LiveKit is not configured"))).toBe("unavailable");
    expect(watchErrorFor(new Error("offline"))).toBe("unavailable");
  });

  it("renders the page without a token: the browser asks for it on Join", () => {
    expect(resolveWatch("0,0", "Viewer", "live")).toEqual({
      status: "live", location: "0,0", isWorld: false, placeName: "Parcel 0,0", identity: "Viewer",
    });
    expect(resolveWatch("example.dcl.eth", "Viewer", "expired")).toMatchObject({
      status: "expired", isWorld: true, error: "access_expired",
    });
  });
});
