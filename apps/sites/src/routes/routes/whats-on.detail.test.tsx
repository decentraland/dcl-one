import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it } from "vitest";
import { type Event } from "@/data/lib/catalyst/places/events";
import EventDetailRoute, { toDialogEvent } from "./whats-on.$id";

const base: Event = { id: "series", name: "Weekly fixture", image: null, image_vertical: null, description: "Bring friends", start_at: "2099-01-01T18:00:00Z", finish_at: "2099-01-01T20:00:00Z", next_start_at: "2099-06-01T18:00:00Z", next_finish_at: null, duration: 7200000, all_day: false, x: 12, y: -3, position: null, coordinates: null, url: null, user_name: null, scene_name: "Plaza", estate_name: null, live: false, highlighted: false, trending: false, recurrent: true, recurrent_frequency: "WEEKLY", total_attendees: 0, place_id: null };

function render(event: Event | null, unavailable = false): string {
  const router = createMemoryRouter([{ id: "detail", path: "/whats-on/:id", Component: EventDetailRoute as never }], { initialEntries: ["/whats-on/series"], hydrationData: { loaderData: { detail: { id: "series", sid: "test", event, unavailable } } } });
  return renderToString(<QueryClientProvider client={new QueryClient()}><RouterProvider router={router} /></QueryClientProvider>);
}

describe("when the public event detail route renders an event signed out", () => {
  let html: string;
  beforeEach(() => { html = render(base); });
  it("should render the event dialog labelled by its title", () => {
    expect(html).toContain('role="dialog"');
    expect(html).toContain('aria-labelledby="event-detail-title"');
    expect(html).toMatch(/id="event-detail-title"[^>]*>Weekly fixture</);
  });
  it("should offer the calendar first and the reminder as a secondary action", () => {
    expect(html).toContain("Add to calendar");
    expect(html).toContain('aria-label="Remind me"');
    expect(html).not.toContain(">Remind me<");
  });
  it("should name the place and its parcel on the jump-in chip", () => {
    expect(html).toContain('aria-label="Jump In – Plaza (12,-3)"');
  });
  it("should show what to expect and the schedule", () => {
    expect(html).toContain("What to expect");
    expect(html).toContain("Bring friends");
    expect(html).toContain("Schedule");
  });
});

describe("when the event is gone", () => {
  it("should say the event was not found", () => {
    expect(render(null)).toContain("Event not found");
  });
});

describe("when the event is in a world", () => {
  it("should map the world's name as the realm", () => {
    const dialog = toDialogEvent({ ...base, world: true, server: "fixture.dcl.eth", recurrent_weekday_mask: 2 + 8 });
    expect(dialog.realm).toBe("fixture.dcl.eth");
    expect(dialog.isWorld).toBe(true);
    expect(dialog.recurrentByDay).toEqual([1, 3]);
  });
});
