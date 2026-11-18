// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";

const fetchSceneRoom = vi.fn();
vi.mock("@/data/lib/comms/scene-chat", async (actual) => ({ ...(await actual<object>()), fetchSceneRoom: (...args: unknown[]) => fetchSceneRoom(...args) }));

type Handler = (payload: Uint8Array, participant?: { identity: string; metadata?: string }) => void;
const rooms: Array<{ connect: ReturnType<typeof vi.fn>; disconnect: ReturnType<typeof vi.fn>; handlers: Handler[] }> = [];
vi.mock("livekit-client", () => ({
  RoomEvent: { DataReceived: "dataReceived" },
  Room: class {
    connect = vi.fn(async () => {});
    disconnect = vi.fn(async () => {});
    handlers: Handler[] = [];
    constructor() {
      rooms.push(this);
    }
    on(_event: string, handler: Handler) {
      this.handlers.push(handler);
    }
  },
}));

const { MAX_CHAT_CHARS, MAX_CHAT_LINES, senderOf, useSceneChat } = await import("./useSceneChat");

const chatPacket = (message: string) => {
  const text = new TextEncoder().encode(message);
  return Uint8Array.from([0x2a, text.length + 2, 0x0a, text.length, ...text]);
};

afterEach(() => {
  fetchSceneRoom.mockReset();
  rooms.length = 0;
});

describe("useSceneChat", () => {
  test("without a target it asks nothing and stays loading", () => {
    const { result } = renderHook(() => useSceneChat(null));
    expect(result.current).toEqual({ status: "loading", messages: [] });
    expect(fetchSceneRoom).not.toHaveBeenCalled();
  });

  test("is unavailable when the gatekeeper has no room for the scene", async () => {
    fetchSceneRoom.mockResolvedValueOnce(null);
    const { result } = renderHook(() => useSceneChat({ location: "1,95" }));
    expect(result.current.status).toBe("loading");
    await waitFor(() => expect(result.current.status).toBe("unavailable"));
    expect(fetchSceneRoom).toHaveBeenCalledWith({ location: "1,95" }, expect.any(AbortSignal));
    expect(rooms).toHaveLength(0);
  });

  test("with a room it is ready, joins it, keeps the chat it hears and leaves on unmount", async () => {
    fetchSceneRoom.mockResolvedValueOnce({ mode: "scene", credentials: { url: "wss://lk.test", token: "jwt" } });
    const { result, unmount } = renderHook(() => useSceneChat({ location: "1,95" }));
    await waitFor(() => expect(result.current.status).toBe("ready"));
    await waitFor(() => expect(rooms).toHaveLength(1));
    expect(rooms[0].connect).toHaveBeenCalledWith("wss://lk.test", "jwt", { autoSubscribe: false });
    act(() => {
      rooms[0].handlers.forEach((h) => h(chatPacket("gm"), { identity: "0xAbC" }));
      rooms[0].handlers.forEach((h) => h(Uint8Array.from([0x0a, 0x00]), { identity: "0xAbC" }));
      rooms[0].handlers.forEach((h) => h(chatPacket("hi"), { identity: "anon:1", metadata: JSON.stringify({ displayName: "keen-otter" }) }));
    });
    expect(result.current.messages.map((m) => [m.sender, m.message])).toEqual([["0xAbC", "gm"], ["keen-otter", "hi"]]);
    unmount();
    expect(rooms[0].disconnect).toHaveBeenCalled();
  });
});

describe("useSceneChat bounds", () => {
  test("keeps the last 200 lines and the first 500 characters of a line", async () => {
    fetchSceneRoom.mockResolvedValueOnce({ mode: "scene", credentials: { url: "wss://lk.test", token: "jwt" } });
    const { result } = renderHook(() => useSceneChat({ location: "1,95" }));
    await waitFor(() => expect(rooms).toHaveLength(1));
    const hear = (payload: Uint8Array) => rooms[0].handlers.forEach((h) => h(payload, { identity: "0xAbC" }));
    // A Chat packet with a 600 byte text: both lengths need a two byte varint.
    const long = new TextEncoder().encode("x".repeat(600));
    act(() => {
      for (let i = 0; i < MAX_CHAT_LINES + 5; i++) hear(chatPacket(`line ${i}`));
      hear(Uint8Array.from([0x2a, 0xdb, 0x04, 0x0a, 0xd8, 0x04, ...long]));
    });
    expect(result.current.messages).toHaveLength(MAX_CHAT_LINES);
    expect(result.current.messages[0].message).toBe("line 6");
    expect(result.current.messages.at(-1)?.message).toBe("x".repeat(MAX_CHAT_CHARS));
  });
});

describe("senderOf", () => {
  test("names a wallet by its address, anyone else by their display name or identity", () => {
    expect(senderOf({ identity: "0x12" })).toBe("0x12");
    expect(senderOf({ identity: "anon:x", metadata: "{bad" })).toBe("anon:x");
    expect(senderOf({ identity: "anon:x", metadata: JSON.stringify({ displayName: "merry-owl" }) })).toBe("merry-owl");
    expect(senderOf(undefined)).toBe("Unknown");
  });
});
