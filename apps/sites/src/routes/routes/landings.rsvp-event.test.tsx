import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";
import { formatEventWhen, type Event } from "@/data/lib/catalyst/places/events";
import RsvpRoute from "./landings.rsvp-event";

describe("when the RSVP route renders a recurring event", () => {
  let event: Event;
  let html: string;
  beforeEach(() => {
    event = { id: "series", name: "Weekly fixture", image: null, image_vertical: null, description: null, start_at: "2099-01-01T18:00:00Z", finish_at: "2099-12-31T20:00:00Z", next_start_at: "2099-06-01T18:00:00Z", next_finish_at: null, duration: 7200000, all_day: false, x: 0, y: 0, position: null, coordinates: null, url: null, user_name: null, scene_name: null, estate_name: null, live: false, highlighted: false, trending: false, recurrent: true, recurrent_frequency: "WEEKLY", total_attendees: 0, place_id: null };
    const router = createMemoryRouter([{ id: "rsvp", path: "/landings/rsvp-event", Component: RsvpRoute as never }], {
      initialEntries: ["/landings/rsvp-event?event=series"],
      hydrationData: { loaderData: { rsvp: {
        sid: "test", step: null, eventId: event.id, assignment: { variant: "confirm" },
        rsvp: { event, attendees: [], count: 0, fromFixture: false },
      } } },
    });
    html = renderToString(<RouterProvider router={router} />);
  });
  it("should display the next occurrence in both the event date and schedule", () => {
    expect(html.split(formatEventWhen(event.next_start_at))).toHaveLength(3);
  });
  it("should not display the original series start", () => {
    expect(html).not.toContain(formatEventWhen(event.start_at));
  });
});
