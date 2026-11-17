import { RouterContextProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fetchEvents, type Event } from "@/data/lib/catalyst/places/events";
import { loader } from "./landings.event-schedule";

vi.mock("@/data/lib/catalyst/places/events", async (original) => ({
  ...await original<typeof import("@/data/lib/catalyst/places/events")>(), fetchEvents: vi.fn(),
}));
vi.mock("@/data/lib/catalyst/landings/schedules", async (original) => ({
  ...await original<typeof import("@/data/lib/catalyst/landings/schedules")>(), fetchSchedules: vi.fn(async () => ({ data: [], total: 0 })),
}));

describe("when the event schedule landing loads live occupancy", () => {
  let event: Event;
  let request: Request;
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2099-06-01T19:00:00Z"));
    event = { id: "live", name: "Live event", start_at: "2099-06-01T18:00:00Z", next_start_at: "2099-06-01T18:00:00Z", connected_addresses: ["0xa", "0xb"], total_attendees: 99 } as Event;
    request = new Request("https://example.test/landings/event-schedule");
    vi.mocked(fetchEvents).mockImplementation(async (params) => ({ data: [{ ...event, connected_addresses: params?.list === "live" ? event.connected_addresses : undefined }], total: 1 }));
  });
  afterEach(() => { vi.resetAllMocks(); vi.useRealTimers(); });
  it("should request and reuse observed wallets for live cards and calendar rows", async () => {
    expect((await loader({ request, context: new RouterContextProvider() } as never)).data).toMatchObject({ liveNow: [{ users: 2 }], allDays: expect.arrayContaining([[expect.objectContaining({ users: 2 })]]) });
    expect(fetchEvents).toHaveBeenCalledWith(expect.objectContaining({ list: "live", with_connected_users: true }), expect.anything());
  });
});
