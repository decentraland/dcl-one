import { describe, expect, it } from "vitest";

import { eventsPageKeys } from "../catalyst/places/events-page";
import type { Event } from "../catalyst/places/events";
import { renderAgentMarkdown } from "./dispatch.server";

const event = {
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
  it("should describe the event an `?id=` link opens, as its old detail page did", () => {
    const md = render({
      tab: null,
      dehydratedState: state(
        { key: eventsPageKeys.detail("e1"), data: event },
        { key: eventsPageKeys.upcoming(), data: [{ ...event, id: "other", name: "Other" }] },
      ),
    });
    expect(md).toContain("Poker Night");
    expect(md).toContain("Cards and chips.");
    expect(md).not.toContain("What's On in Decentraland");
  });

  it("should list live and upcoming events without an event link", () => {
    const md = render({ tab: null, dehydratedState: state({ key: eventsPageKeys.upcoming(), data: [event] }) });
    expect(md).toContain("What's On in Decentraland");
    expect(md).toContain("[Poker Night](/events?id=e1)");
  });

  it("should fall back to the list when the linked event was not found", () => {
    const md = render({ tab: null, dehydratedState: state({ key: eventsPageKeys.detail("gone"), data: null }) });
    expect(md).toContain("What's On in Decentraland");
  });
});
