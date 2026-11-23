import { beforeEach, describe, expect, it } from "vitest";

import { eventsPageKeys } from "../catalyst/places/events-page";
import type { Event } from "../catalyst/places/events";
import { renderAgentMarkdown } from "./dispatch.server";

function render(data: unknown) {
  const context = {
    staticHandlerContext: {
      matches: [{ route: { id: "events", handle: { agentMarkdown: "whatsOn" } } }],
      loaderData: { events: data },
      statusCode: 200,
    },
  };
  return renderAgentMarkdown(context as never)?.md ?? "";
}

const state = (...queries: { key: readonly unknown[]; data: unknown }[]) => ({
  mutations: [],
  queries: queries.map(({ key, data }) => ({
    queryKey: key,
    queryHash: JSON.stringify(key),
    state: { data, status: "success" },
  })),
});

describe("the /events agent markdown", () => {
  let event: Event;
  let md: string;

  beforeEach(() => {
    event = {
      id: "e1",
      name: "Poker Night",
      description: "Cards and chips.",
      start_at: "2099-06-01T18:00:00Z",
      next_start_at: "2099-06-01T18:00:00Z",
      x: 10,
      y: 20,
      user_name: "OtoLam",
      live: false,
      recurrent: false,
      total_attendees: 3,
    } as Event;
  });

  describe("when an `?id=` link opens an event that exists", () => {
    beforeEach(() => {
      md = render({
        tab: null,
        dehydratedState: state(
          { key: eventsPageKeys.detail("e1"), data: event },
          { key: eventsPageKeys.upcoming(), data: [{ ...event, id: "other", name: "Other" }] },
        ),
      });
    });

    it("should describe the event, as its old detail page did", () => {
      expect(md).toContain("Poker Night");
      expect(md).toContain("Cards and chips.");
    });

    it("should not render the events list", () => {
      expect(md).not.toContain("What's On in Decentraland");
    });
  });

  describe("when there is no event link", () => {
    beforeEach(() => {
      md = render({ tab: null, dehydratedState: state({ key: eventsPageKeys.upcoming(), data: [event] }) });
    });

    it("should render the What's On heading", () => {
      expect(md).toContain("What's On in Decentraland");
    });

    it("should link each live and upcoming event", () => {
      expect(md).toContain("[Poker Night](/events?id=e1)");
    });
  });

  describe("when the linked event was not found", () => {
    beforeEach(() => {
      md = render({ tab: null, dehydratedState: state({ key: eventsPageKeys.detail("gone"), data: null }) });
    });

    it("should fall back to the list", () => {
      expect(md).toContain("What's On in Decentraland");
    });
  });
});
