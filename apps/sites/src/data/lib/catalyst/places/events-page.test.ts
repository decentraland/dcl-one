import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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
  describe("when keying event details", () => {
    it("should give every event its own entry", () => {
      expect(eventsPageKeys.detail("a")).not.toEqual(eventsPageKeys.detail("b"));
    });
  });

  describe("when keying place targets", () => {
    it("should give every position its own entry", () => {
      expect(eventsPageKeys.place({ position: [1, 2] })).not.toEqual(eventsPageKeys.place({ position: [2, 1] }));
    });

    it("should keep a World apart from a position", () => {
      expect(eventsPageKeys.place({ world: "a.dcl.eth" })).not.toEqual(eventsPageKeys.place({ position: [0, 0] }));
    });
  });

  describe("when keying the page's public reads", () => {
    it.each([
      ["live", () => eventsPageKeys.live()],
      ["upcoming", () => eventsPageKeys.upcoming()],
      ["calendar", () => eventsPageKeys.calendar()],
    ])("should keep the %s key out of the private prefix", (_name, key) => {
      expect(key()[0]).toBe("events");
    });
  });
});

describe("readPlaceTarget", () => {
  describe("when the link carries a bare World name, as older share links do", () => {
    it("should expand it", () => {
      expect(readPlaceTarget(new URLSearchParams("world=Brai"))).toEqual({ world: "brai.dcl.eth" });
    });
  });

  describe("when the link carries both a World and a position", () => {
    it("should prefer the World", () => {
      expect(readPlaceTarget(new URLSearchParams("world=a.eth&position=1,2"))).toEqual({ world: "a.eth" });
    });
  });

  describe("when the position uses the dot separator", () => {
    it("should read it", () => {
      expect(readPlaceTarget(new URLSearchParams("position=-3.4"))).toEqual({ position: [-3, 4] });
    });
  });

  describe("when the target is malformed", () => {
    it.each(["position=1", "position=a,b", "world=bad name!", "position=1000,1", ""])("should ignore %j", (query) => {
      expect(readPlaceTarget(new URLSearchParams(query))).toBeNull();
    });
  });
});

describe("readEventId", () => {
  describe("when the id is blank", () => {
    it("should read nothing", () => {
      expect(readEventId(new URLSearchParams("id=%20"))).toBeNull();
    });
  });

  describe("when the id is not blank", () => {
    it("should read it", () => {
      expect(readEventId(new URLSearchParams("id=abc"))).toBe("abc");
    });
  });
});

describe("fetchEventDetail", () => {
  describe("when the events service does not know the id", () => {
    beforeEach(() => {
      vi.spyOn(events, "fetchEvent").mockRejectedValueOnce(new CatalystError("missing", "/events", 404));
    });

    it("should answer null", async () => {
      await expect(fetchEventDetail("nope")).resolves.toBeNull();
    });
  });

  describe("when the events service is down", () => {
    beforeEach(() => {
      vi.spyOn(events, "fetchEvent").mockRejectedValueOnce(new CatalystError("down", "/events", 503));
    });

    it("should throw the outage, so the page keeps the link instead of closing it", async () => {
      await expect(fetchEventDetail("e1")).rejects.toThrow("down");
    });
  });

  describe("when the event was rejected or deleted", () => {
    beforeEach(() => {
      vi.spyOn(events, "fetchEvent").mockResolvedValueOnce(event({ deleted_by_admin: true }));
    });

    it("should not open it", async () => {
      await expect(fetchEventDetail("e1")).resolves.toBeNull();
    });
  });
});

describe("fetchUpcoming", () => {
  describe("when the service answers upcoming, live and unapproved events", () => {
    let read: ReturnType<typeof vi.spyOn>;
    let result: Promise<unknown>;

    beforeEach(() => {
      read = vi.spyOn(events, "fetchEvents").mockResolvedValueOnce({
        data: [event({ id: "soon" }), event({ id: "now", live: true }), event({ id: "pending", approved: false })],
        total: 3,
      });
      result = fetchUpcoming({}, Date.parse("2099-06-01T12:00:00Z"));
    });

    it("should leave out live and unapproved events", async () => {
      await expect(result).resolves.toMatchObject([{ id: "soon" }]);
    });

    it("should ask for the next day of upcoming events", async () => {
      await result;
      expect(read).toHaveBeenCalledWith(
        expect.objectContaining({ list: "upcoming", from: "2099-06-01T12:00:00.000Z", to: "2099-06-02T12:00:00.000Z" }),
        {},
      );
    });
  });
});

describe("fetchLiveNow", () => {
  describe("when the places read fails", () => {
    beforeEach(() => {
      vi.spyOn(events, "fetchEvents").mockResolvedValueOnce({ data: [], total: 0 });
      vi.spyOn(places, "fetchMostActivePlaces").mockRejectedValueOnce(new CatalystError("down", "/places", 502));
    });

    it("should fail rather than show the rail without places", async () => {
      await expect(fetchLiveNow()).rejects.toThrow("down");
    });
  });
});

describe("eventsAgentData", () => {
  describe("when the page dehydrated live and upcoming events", () => {
    let state: Parameters<typeof eventsAgentData>[0];

    beforeEach(() => {
      state = {
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
    });

    it("should read them", () => {
      expect(eventsAgentData(state)).toMatchObject({ live: [{ id: "l" }], upcoming: [{ id: "u" }] });
    });
  });

  describe("when the state is empty", () => {
    it("should read nothing", () => {
      expect(eventsAgentData(null)).toEqual({ live: [], upcoming: [] });
    });
  });
});
