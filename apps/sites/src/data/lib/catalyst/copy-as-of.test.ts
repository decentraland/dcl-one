// A mirrored catalogue's answer carries `as_of`, its mirror's last complete pass, and `stale_after_secs`,
// the node's budget for it (services/catalyst/shared/src/as_of.rs); the loaders hand both to the pages'
// as-of notice, with the rows they describe.
import { describe, expect, it, vi } from "vitest";

import { apiSource } from "./api";
import { fetchEvents } from "./places/events";
import { placesCopy } from "./places/index.server";
import { loadCommunity } from "./social/communities";

const AS_OF = "2026-09-30T08:00:00.500Z";
const COPY = { asOf: Date.parse(AS_OF), staleAfterMs: 7_200_000 };
const COMMUNITY = "0b3e3a64-8c3f-4b1f-9f0b-1c2d3e4f5a6b";

function answering(body: (path: string) => unknown): typeof fetch {
  return (async (input: RequestInfo | URL) => {
    const path = new URL(String(input), "http://node.invalid").pathname;
    return new Response(JSON.stringify(body(path)), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  }) as typeof fetch;
}

const stamped = (body: object, asOf = AS_OF) => ({ as_of: asOf, stale_after_secs: 7200, ...body });

describe("the copy's stamp reaches the pages", () => {
  it("places and events carry the stamp and its budget, a live answer none", async () => {
    const places = await apiSource.fetchPlaces({}, { fetchImpl: answering(() => stamped({ ok: true, data: [], total: 0 })) });
    expect(places.copy).toEqual(COPY);
    const live = await apiSource.fetchPlaces({}, { fetchImpl: answering(() => ({ ok: true, data: [], total: 0 })) });
    expect(live.copy).toBeNull();

    const events = await fetchEvents({ list: "active" }, { fetchImpl: answering(() => stamped({ ok: true, data: [], total: 0 })) });
    expect(events.copy).toEqual(COPY);
  });

  it("rows read straight from the database take the API's stamp, probed once a minute", async () => {
    const T = Date.parse("2026-10-02T12:00:00Z");
    const first = await placesCopy({ fetchImpl: answering(() => stamped({ ok: true, data: [], total: 0 })) }, T);
    expect(first).toEqual(COPY);
    const reused = await placesCopy({ fetchImpl: answering(() => ({ ok: true, data: [], total: 0 })) }, T + 59_000);
    expect(reused, "within the minute: the probe is reused").toEqual(COPY);
    const again = await placesCopy({ fetchImpl: answering(() => ({ ok: true, data: [], total: 0 })) }, T + 60_000);
    expect(again, "a minute on: probed again, and the node now answers live").toBeNull();
  });

  it("a probe that failed, or whose first caller gave up, leaves the next request its stamp", async () => {
    const T = Date.parse("2026-10-09T12:00:00Z");
    // The node answers the stamp to any fetch it is let finish.
    const honouring = (async (_input: RequestInfo | URL, init?: RequestInit) => {
      if (init?.signal?.aborted) throw new DOMException("aborted", "AbortError");
      return answering(() => stamped({ ok: true, data: [], total: 0 }))(_input, init);
    }) as typeof fetch;
    const gaveUp = new AbortController();
    gaveUp.abort();
    const first = await placesCopy({ fetchImpl: honouring, signal: gaveUp.signal }, T);
    expect(first, "the probe is not the first caller's to cancel").toEqual(COPY);
    const second = await placesCopy({ fetchImpl: honouring }, T + 1_000);
    expect(second, "the next request in the minute still has the notice").toEqual(COPY);

    const down = (async () => {
      throw new TypeError("fetch failed");
    }) as typeof fetch;
    const failed = await placesCopy({ fetchImpl: down }, T + 60_000);
    expect(failed).toBeNull();
    const next = await placesCopy({ fetchImpl: honouring }, T + 61_000);
    expect(next, "a failed probe is not reused").toEqual(COPY);
  });

  it("a node that never answers holds the rows no longer than the probe's guard", async () => {
    vi.useFakeTimers();
    try {
      const hanging = (async (_input: RequestInfo | URL, init?: RequestInit) =>
        new Promise<Response>((_, reject) =>
          init?.signal?.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError"))),
        )) as typeof fetch;
      let settled = false;
      const probe = placesCopy({ fetchImpl: hanging }, Date.parse("2026-10-16T12:00:00Z")).finally(() => {
        settled = true;
      });
      await vi.advanceTimersByTimeAsync(4_999);
      expect(settled, "still waiting inside the guard").toBe(false);
      await vi.advanceTimersByTimeAsync(1);
      await expect(probe).resolves.toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("a mirrored community carries its mirror's stamp, the node's own none", async () => {
    const community = { id: COMMUNITY, name: "Builders", ownerAddress: "0xabcabcabcabcabcabcabcabcabcabcabcabcabca", privacy: "public" };
    const mirrored = await loadCommunity(COMMUNITY, {
      fetchImpl: answering((path) => (path.endsWith("/members") ? { data: { results: [] } } : stamped({ data: community }))),
    });
    expect(mirrored?.copy).toEqual(COPY);
    const own = await loadCommunity(COMMUNITY, {
      fetchImpl: answering((path) => (path.endsWith("/members") ? { data: { results: [] } } : { data: community })),
    });
    expect(own?.community.id).toBe(COMMUNITY);
    expect(own?.copy).toBeNull();
  });
});
