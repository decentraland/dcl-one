import { afterEach, describe, expect, it, vi } from "vitest";

import { CatalystError } from "../client";
import * as events from "./events";
import * as places from "./index";
import {
  eventsAgentData,
  eventsPageKeys,
  fetchEventDetail,
  fetchLiveNow,
  fetchUpcoming,
  readEventId,
  readPlaceTarget,
} from "./events-page";

afterEach(() => vi.restoreAllMocks());

const event = (overrides: Partial<events.Event>) =>
  ({ id: "e1", live: false, approved: true, ...overrides }) as events.Event;

describe("eventsPageKeys", () => {
  it("should give every event and every place target its own entry", () => {
    expect(eventsPageKeys.detail("a")).not.toEqual(eventsPageKeys.detail("b"));
    expect(eventsPageKeys.place({ position: [1, 2] })).not.toEqual(eventsPageKeys.place({ position: [2, 1] }));
    expect(eventsPageKeys.place({ world: "a.dcl.eth" })).not.toEqual(eventsPageKeys.place({ position: [0, 0] }));
  });

  it("should keep the page's public keys out of the private prefix", () => {
    for (const key of [eventsPageKeys.live(), eventsPageKeys.upcoming(), eventsPageKeys.calendar()])
      expect(key[0]).toBe("events");
  });
});

describe("readPlaceTarget", () => {
  it("should expand a bare World name, as older share links carry it", () => {
    expect(readPlaceTarget(new URLSearchParams("world=Brai"))).toEqual({ world: "brai.dcl.eth" });
  });

  it("should prefer the World over a position and read either separator", () => {
    expect(readPlaceTarget(new URLSearchParams("world=a.eth&position=1,2"))).toEqual({ world: "a.eth" });
    expect(readPlaceTarget(new URLSearchParams("position=-3.4"))).toEqual({ position: [-3, 4] });
  });

  it("should ignore a malformed target", () => {
    for (const query of ["position=1", "position=a,b", "world=bad name!", "position=1000,1", ""]) {
      expect(readPlaceTarget(new URLSearchParams(query))).toBeNull();
    }
  });

  it("should read an event id only when it is not blank", () => {
    expect(readEventId(new URLSearchParams("id=%20"))).toBeNull();
    expect(readEventId(new URLSearchParams("id=abc"))).toBe("abc");
  });
});

describe("fetchEventDetail", () => {
  it("should answer null for an id the events service does not know", async () => {
    vi.spyOn(events, "fetchEvent").mockRejectedValue(new CatalystError("missing", "/events", 404));
    await expect(fetchEventDetail("nope")).resolves.toBeNull();
  });

  it("should throw an outage, so the page keeps the link instead of closing it", async () => {
    vi.spyOn(events, "fetchEvent").mockRejectedValue(new CatalystError("down", "/events", 503));
    await expect(fetchEventDetail("e1")).rejects.toThrow("down");
  });

  it("should not open a rejected or deleted event", async () => {
    vi.spyOn(events, "fetchEvent").mockResolvedValue(event({ deleted_by_admin: true }));
    await expect(fetchEventDetail("e1")).resolves.toBeNull();
  });
});

describe("fetchUpcoming", () => {
  it("should ask for the next day of upcoming events and leave out live and unapproved ones", async () => {
    const read = vi.spyOn(events, "fetchEvents").mockResolvedValue({
      data: [event({ id: "soon" }), event({ id: "now", live: true }), event({ id: "pending", approved: false })],
      total: 3,
    });
    const now = Date.parse("2099-06-01T12:00:00Z");
    await expect(fetchUpcoming({}, now)).resolves.toMatchObject([{ id: "soon" }]);
    expect(read).toHaveBeenCalledWith(
      expect.objectContaining({ list: "upcoming", from: "2099-06-01T12:00:00.000Z", to: "2099-06-02T12:00:00.000Z" }),
      {},
    );
  });
});

describe("fetchLiveNow", () => {
  it("should fail when the places read fails, rather than show the rail without places", async () => {
    vi.spyOn(events, "fetchEvents").mockResolvedValue({ data: [], total: 0 });
    vi.spyOn(places, "fetchMostActivePlaces").mockRejectedValue(new CatalystError("down", "/places", 502));
    await expect(fetchLiveNow()).rejects.toThrow("down");
  });
});

describe("eventsAgentData", () => {
  it("should read the live and upcoming events the page dehydrated, and nothing from an empty state", () => {
    const state = {
      mutations: [],
      queries: [
        {
          queryKey: eventsPageKeys.live(),
          queryHash: "",
          state: { data: { events: [event({ id: "l" })], places: [] } },
        },
        { queryKey: eventsPageKeys.upcoming(), queryHash: "", state: { data: [event({ id: "u" })] } },
      ],
    } as never;
    expect(eventsAgentData(state)).toMatchObject({ live: [{ id: "l" }], upcoming: [{ id: "u" }] });
    expect(eventsAgentData(null)).toEqual({ live: [], upcoming: [] });
  });
});
