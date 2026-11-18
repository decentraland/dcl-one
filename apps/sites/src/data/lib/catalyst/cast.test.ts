import { afterEach, describe, expect, it, vi } from "vitest";

const { postJSON } = vi.hoisted(() => ({ postJSON: vi.fn() }));
vi.mock("./client", async (orig) => ({ ...(await orig<typeof import("./client")>()), postJSON }));

import type { AuthIdentity } from "../auth/types";
import { CatalystError } from "./client";
import { defaultDisplayName, getStreamInfo, getStreamerToken, getWatcherToken } from "./cast";

const BASE = "https://node.test";
const json = (status: number, body: unknown) =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

afterEach(() => postJSON.mockReset());

describe("Cast 2.0 client (upstream gatekeeper contract)", () => {
  it("reads stream info by streaming key, unauthenticated", async () => {
    const fetchImpl = vi.fn(async () =>
      json(200, { placeName: "Plaza", placeId: "p1", location: "1,2", isWorld: false }),
    );
    const info = await getStreamInfo("key/1", { base: BASE, fetchImpl });
    expect(info).toEqual({ placeName: "Plaza", placeId: "p1", location: "1,2", isWorld: false });
    const [url, init] = fetchImpl.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe(`${BASE}/cast/stream-info/key%2F1`);
    expect(init.method).toBe("GET");
    expect(new Headers(init.headers).has("x-identity-auth-chain-0")).toBe(false);
  });

  it("surfaces the gatekeeper's {error} message and status", async () => {
    const fetchImpl = vi.fn(async () => json(400, { error: "Invalid streaming key" }));
    const call = getStreamInfo("nope", { base: BASE, fetchImpl });
    await expect(call).rejects.toBeInstanceOf(CatalystError);
    await expect(call).rejects.toMatchObject({ message: "Invalid streaming key", status: 400 });
  });

  it("answers a 200 that is not JSON as a CatalystError", async () => {
    const fetchImpl = vi.fn(async () => new Response("<html>", { status: 200 }));
    const call = getStreamInfo("k", { base: BASE, fetchImpl });
    await expect(call).rejects.toBeInstanceOf(CatalystError);
    await expect(call).rejects.toMatchObject({ status: 200 });
  });

  it("posts {token, identity} for the streamer token", async () => {
    const fetchImpl = vi.fn(async () =>
      json(200, { url: "wss://lk", token: "jwt", roomId: "scene-room", identity: "stream:p1:u" }),
    );
    const creds = await getStreamerToken("k1", "Ada", { base: BASE, fetchImpl });
    expect(creds.roomId).toBe("scene-room");
    const [url, init] = fetchImpl.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe(`${BASE}/cast/streamer-token`);
    expect(init.method).toBe("POST");
    expect(JSON.parse(String(init.body))).toEqual({ token: "k1", identity: "Ada" });
  });

  it("signs the watcher token with the viewer's identity and sends parcel only when given", async () => {
    const identity = { signer: "0xabc" } as unknown as AuthIdentity;
    postJSON.mockResolvedValue({ url: "wss://lk", token: "jwt", roomId: "r", identity: "watch:r:u", placeName: "P" });
    const creds = await getWatcherToken(identity, { location: "name.dcl.eth", displayName: "Guest#0abc" });
    expect(creds.placeName).toBe("P");
    expect(postJSON).toHaveBeenLastCalledWith(
      "/cast/watcher-token",
      { location: "name.dcl.eth", identity: "Guest#0abc" },
      expect.objectContaining({ identity }),
    );
    await getWatcherToken(identity, { location: "name.dcl.eth", displayName: "G", parcel: "1,4" });
    expect(postJSON.mock.lastCall?.[1]).toEqual({ location: "name.dcl.eth", identity: "G", parcel: "1,4" });
  });

  it("names a viewer by the tail of the wallet", () => {
    expect(defaultDisplayName("0x00000000000000000000000000000000000ABCD1")).toBe("Guest#bcd1");
    expect(defaultDisplayName(null)).toBe("Guest");
  });
});
