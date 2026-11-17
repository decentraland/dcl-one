import { readFileSync } from "node:fs";
import { afterEach, describe, expect, test, vi } from "vitest";

import type { AuthIdentity } from "../auth/types";

const signedFetch = vi.fn();
vi.mock("../auth/signer", () => ({ signedFetch: (...args: unknown[]) => signedFetch(...args) }));
vi.mock("../catalyst/client", () => ({
  catalystBase: () => "https://node.test",
  getJSON: async (path: string, opts: { signal?: AbortSignal }) => (await fetch(`https://node.test${path}`, opts)).json(),
}));

const { decodeChatPacket, fetchSceneRoom, generateRandomName, parseAdapterUrl } = await import("./scene-chat");

const wallet: AuthIdentity = { signer: "0xabc", ephemeral: { address: "0xeph", privateKey: "0x01" }, expiration: "2999-01-01T00:00:00.000Z", authChain: [] };
const about = { configurations: { realmName: "node-realm" } };
const json = (status: number, body: unknown) => new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });

/** A protobuf field: tag (field << 3 | wire type 2), length, bytes. */
const field = (n: number, bytes: Uint8Array) => Uint8Array.from([(n << 3) | 2, bytes.length, ...bytes]);
const utf8 = (s: string) => new TextEncoder().encode(s);
const chatPacket = (message: string, extra: number[] = []) => field(5, Uint8Array.from([...field(1, utf8(message)), ...extra]));

afterEach(() => {
  vi.restoreAllMocks();
  signedFetch.mockReset();
});

const COMMS_PROTO = new URL("../../../../../../library/protocol/proto/decentraland/kernel/comms/rfc4/comms.proto", import.meta.url);

describe("decodeChatPacket", () => {
  test("reads the field numbers the proto declares for Packet.chat and Chat.message", () => {
    const proto = readFileSync(COMMS_PROTO, "utf8");
    const packetChat = Number(/^\s*Chat chat = (\d+);/m.exec(proto)?.[1]);
    const chatMessage = Number(/message Chat \{[^}]*?\bstring message = (\d+);/.exec(proto)?.[1]);
    expect([packetChat, chatMessage]).toEqual([5, 1]);
    expect(decodeChatPacket(field(packetChat, field(chatMessage, utf8("gm"))))).toBe("gm");
  });

  test("reads the text of a chat packet, skipping the other fields", () => {
    // Chat.timestamp is a fixed64 (wire type 1) and Chat.forwarded_from a string; both must be stepped over.
    const timestamp = [0x11, 0, 0, 0, 0, 0, 0, 0xf0, 0x3f];
    expect(decodeChatPacket(chatPacket("hola", [...timestamp, ...field(3, utf8("0xfwd"))]))).toBe("hola");
    expect(decodeChatPacket(Uint8Array.from([0x08, 0x64, ...chatPacket("with protocol version")]))).toBe("with protocol version");
  });

  test("is null for another packet, a control message or bytes that are not a packet", () => {
    expect(decodeChatPacket(field(1, Uint8Array.from([0x0d, 0, 0, 0, 0])))).toBeNull();
    expect(decodeChatPacket(chatPacket("␐emote"))).toBeNull();
    expect(decodeChatPacket(Uint8Array.from([0x2a, 0x7f, 0x01]))).toBeNull();
  });
});

describe("parseAdapterUrl", () => {
  test("splits the gatekeeper's livekit adapter into the server and its token", () => {
    expect(parseAdapterUrl("livekit:wss://lk.test/room?access_token=t0k&x=1")).toEqual({ url: "wss://lk.test/room?x=1", token: "t0k" });
    expect(parseAdapterUrl("wss://lk.test/room")).toBeNull();
    expect(parseAdapterUrl("not a url")).toBeNull();
  });
});

describe("generateRandomName", () => {
  test("draws the adjective then the noun from Math.random, as the reference does", () => {
    const random = vi.spyOn(Math, "random").mockReturnValueOnce(1 / 30).mockReturnValueOnce(22 / 30);
    expect(generateRandomName()).toBe("brave-lynx");
    expect(random).toHaveBeenCalledTimes(2);
  });
});

describe("fetchSceneRoom", () => {
  test("resolves the parcel's scene and the node's realm, asks the gatekeeper signed as a guest and parses the adapter", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(json(200, [{ id: "bafyscene" }])).mockResolvedValueOnce(json(200, about));
    signedFetch.mockResolvedValueOnce(json(200, { adapter: "livekit:wss://lk.test?access_token=jwt" }));
    await expect(fetchSceneRoom({ location: "1,95" })).resolves.toEqual({ mode: "scene", credentials: { url: "wss://lk.test/", token: "jwt" } });
    expect(fetchMock).toHaveBeenLastCalledWith("https://node.test/about", expect.anything());
    expect(fetchMock).toHaveBeenCalledWith("https://node.test/content/entities/active", expect.objectContaining({ method: "POST", body: JSON.stringify({ pointers: ["1,95"] }) }));
    expect(signedFetch).toHaveBeenCalledWith(expect.objectContaining({ signer: expect.stringMatching(/^0x[0-9a-f]{40}$/) }), "https://node.test/comms-gatekeeper/get-scene-adapter", expect.objectContaining({
      method: "POST",
      metadata: { sceneId: "bafyscene", parcel: "1,95", realmName: "node-realm", signer: "decentraland-kernel-scene" },
      body: "{}",
    }));
  });

  test("a guest identity signs for a Genesis City parcel, never the session's wallet", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(json(200, [{ id: "bafyscene" }])).mockResolvedValueOnce(json(200, about));
    signedFetch.mockResolvedValueOnce(json(200, { adapter: "livekit:wss://lk.test?access_token=jwt" }));
    await expect(fetchSceneRoom({ location: "-3,7" })).resolves.toMatchObject({ mode: "scene" });
    const [guest] = signedFetch.mock.calls[0] as [AuthIdentity];
    expect(guest.signer).toMatch(/^0x[0-9a-f]{40}$/);
    expect(guest.signer).not.toBe(wallet.signer);
    expect(guest.authChain.map((l) => l.type)).toEqual(["SIGNER", "ECDSA_EPHEMERAL"]);
  });

  test("a world has no room: neither the scene adapter nor a cast token is asked for", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch");
    for (const location of ["cozy.dcl.eth", "Cozy.DCL.ETH", "cozy", ""]) await expect(fetchSceneRoom({ location })).resolves.toBeNull();
    expect(fetchMock).not.toHaveBeenCalled();
    expect(signedFetch).not.toHaveBeenCalled();
  });

  test("without a scene room it asks for a cast watcher token, and without one there is no room", async () => {
    vi.spyOn(Math, "random").mockReturnValue(1 / 30);
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(json(200, [{ id: "bafyscene" }])).mockResolvedValueOnce(json(200, about)).mockResolvedValueOnce(json(501, { error: "not ported" }));
    signedFetch.mockResolvedValueOnce(json(401, { error: "denied" }));
    await expect(fetchSceneRoom({ location: "1,95" })).resolves.toBeNull();
    expect(fetchMock).toHaveBeenLastCalledWith("https://node.test/comms-gatekeeper/cast/watcher-token", expect.objectContaining({ method: "POST", body: JSON.stringify({ location: "1,95", identity: "brave-falcon" }) }));
  });

  test("a gatekeeper answer without an adapter, or a node without a realm name, is no scene room", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(json(200, [{ id: "bafyscene" }])).mockResolvedValueOnce(json(200, about)).mockResolvedValueOnce(json(501, {}));
    signedFetch.mockResolvedValueOnce(json(200, { adapter: 7 }));
    await expect(fetchSceneRoom({ location: "1,95" })).resolves.toBeNull();

    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(json(200, [{ id: "bafyscene" }])).mockResolvedValueOnce(json(200, { configurations: {} })).mockResolvedValueOnce(json(501, {}));
    signedFetch.mockClear();
    await expect(fetchSceneRoom({ location: "1,95" })).resolves.toBeNull();
    expect(signedFetch).not.toHaveBeenCalled();
  });

  test("drops the body of each answer with an error status", async () => {
    const [noAdapter, noCast, noEntity] = [json(401, { error: "denied" }), json(501, { error: "not ported" }), json(503, { error: "down" })];
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(json(200, [{ id: "bafyscene" }])).mockResolvedValueOnce(json(200, about)).mockResolvedValueOnce(noCast).mockResolvedValueOnce(noEntity).mockResolvedValueOnce(json(501, {}));
    signedFetch.mockResolvedValueOnce(noAdapter);
    await expect(fetchSceneRoom({ location: "1,95" })).resolves.toBeNull();
    await expect(fetchSceneRoom({ location: "1,95" })).resolves.toBeNull();
    expect([noAdapter.bodyUsed, noCast.bodyUsed, noEntity.bodyUsed]).toEqual([true, true, true]);
  });

  test("a cast broadcast is the room when the scene has none", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(json(404, [])).mockResolvedValueOnce(json(200, { url: "wss://cast.test", token: "cast-jwt" }));
    await expect(fetchSceneRoom({ location: "1,95" })).resolves.toEqual({ mode: "cast", credentials: { url: "wss://cast.test", token: "cast-jwt" } });
    expect(signedFetch).not.toHaveBeenCalled();
  });
});
