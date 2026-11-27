import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { loadHomeLive } from "./home.server";

afterEach(() => vi.useRealTimers());

const json = (body: unknown) =>
  new Response(JSON.stringify(body), {
    headers: { "content-type": "application/json" },
  });

/** A request the node accepts and never answers; it ends only when its signal aborts. */
function hang(signals: AbortSignal[], init?: RequestInit): Promise<Response> {
  const signal = init?.signal as AbortSignal;
  signals.push(signal);
  return new Promise((_resolve, reject) => {
    signal.addEventListener("abort", () => reject(signal.reason), {
      once: true,
    });
  });
}

describe("Home Live Now read", () => {
  it("aborts a node that does not answer at the 2 s deadline and renders without cards", async () => {
    vi.useFakeTimers();
    const signals: AbortSignal[] = [];
    const fetchImpl = vi.fn((_url: string | URL | Request, init?: RequestInit) =>
      hang(signals, init),
    );

    const pending = loadHomeLive({ fetchImpl: fetchImpl as typeof fetch });
    await vi.advanceTimersByTimeAsync(1_999);
    expect(signals.some((signal) => signal.aborted)).toBe(false);
    await vi.advanceTimersByTimeAsync(1);
    const result = await pending;

    expect(signals).toHaveLength(2);
    expect(signals.every((signal) => signal.aborted)).toBe(true);
    expect(result.cards).toEqual([]);
    expect(result.live).toBe(false);
    expect(result.unreadable).toEqual(["events"]);
  });

  it("keeps the cards it read when the creators' read passes the deadline", async () => {
    vi.useFakeTimers();
    const signals: AbortSignal[] = [];
    const fetchImpl = vi.fn((url: string | URL | Request, init?: RequestInit) => {
      const path = new URL(String(url)).pathname;
      if (path.endsWith("/v1/events")) return Promise.resolve(json({ data: [] }));
      if (path.endsWith("/hot-scenes")) return Promise.resolve(json([]));
      return hang(signals, init);
    });

    const pending = loadHomeLive({ fetchImpl: fetchImpl as typeof fetch });
    await vi.advanceTimersByTimeAsync(2_000);
    const result = await pending;

    expect(signals.length).toBeGreaterThan(0);
    expect(signals.every((signal) => signal.aborted)).toBe(true);
    expect(result.cards).toHaveLength(1);
    expect(result.profiles).toEqual({});
    expect(result.unreadable).toEqual(["profiles"]);
  });
});

describe("when Home reads a live event in a busy scene", () => {
  const attendee = "0xABCDEF1234567890123456789012345678901234";
  const occurrence = "2026-11-27T10:00:00.000Z";
  let fetchImpl: ReturnType<typeof vi.fn<typeof fetch>>;
  let result: Awaited<ReturnType<typeof loadHomeLive>>;

  beforeEach(async () => {
    fetchImpl = vi.fn<typeof fetch>(async (url) => {
      const path = new URL(String(url));
      if (path.pathname.endsWith("/v1/events")) {
        return json({ data: [{
          id: "live",
          name: "Live event",
          x: 1,
          y: 2,
          start_at: "2026-11-20T10:00:00.000Z",
          next_start_at: occurrence,
          latest_attendees: [attendee, attendee.toLowerCase(), "invalid-address"],
          connected_addresses: ["0x9999999999999999999999999999999999999999"],
        }] });
      }
      if (path.pathname.endsWith("/hot-scenes")) {
        return json([{
          id: "scene",
          name: "Scene",
          baseCoords: [1, 2],
          usersTotalCount: 8,
          parcels: [[1, 2]],
        }]);
      }
      return json([]);
    });
    result = await loadHomeLive({ base: "https://node.example", fetchImpl });
  });

  it("should use the public event list without requesting live player identities", () => {
    const eventRequest = fetchImpl.mock.calls.find(([url]) =>
      new URL(String(url)).pathname.endsWith("/v1/events"),
    );
    expect(eventRequest?.[0]).toEqual(expect.stringContaining("list=live"));
    expect(eventRequest?.[0]).not.toEqual(expect.stringContaining("with_connected_users"));
  });

  it("should use the active occurrence and deduplicate public RSVP addresses", () => {
    expect(result.cards[0]).toMatchObject({
      id: "live",
      startedAt: occurrence,
      attendeeAddresses: [attendee.toLowerCase()],
    });
  });

  it("should batch the public attendee's profile with the other Home profiles", () => {
    const profileRequest = fetchImpl.mock.calls.find(([url]) =>
      String(url).endsWith("/lambdas/profiles"),
    );
    expect(profileRequest?.[1]?.body).toEqual(expect.stringContaining(attendee.toLowerCase()));
    expect(profileRequest?.[1]?.body).not.toEqual(expect.stringContaining("0x9999999999999999999999999999999999999999"));
  });
});
