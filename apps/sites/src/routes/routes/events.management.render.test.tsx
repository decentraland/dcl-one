import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { describe, expect, it, vi } from "vitest";

import { whatsOnToMarkdown } from "@/data/lib/agent/markdown";
import Events from "./events";

vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: null, address: null, isConnected: false }) }));

function renderAt(path: string) {
  const router = createMemoryRouter([{ id: "events", path: "/events", Component: Events as never }], {
    initialEntries: [path],
    hydrationData: { loaderData: { events: null } },
  });
  return renderToString(
    <QueryClientProvider client={new QueryClient()}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  ).replace(/<!--.*?-->/g, "");
}

describe("when /events opens My Events", () => {
  it("should render the Events page itself, not a separate management page", () => {
    const html = renderAt("/events?tab=my");
    expect(html).toContain(">All Events</h2>");
    expect(html).toContain("Host an Event");
    expect(html).not.toContain("My Hangouts");
  });

  it("should describe the public page and point agents at the signed-in My Events section", () => {
    const md = whatsOnToMarkdown({ tab: "my" });
    expect(md).toContain("# What's On in Decentraland");
    expect(md).toContain("## My Events");
    expect(md).toContain("(/events?tab=my)");
  });
});
