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
  return renderToString(<RouterProvider router={router} />).replace(/<!--.*?-->/g, "");
}

describe("when /events opens My Events", () => {
  it("should render the My Hangouts management page with its sign-in action", () => {
    const html = renderAt("/events?tab=my");
    expect(html).toContain("Sign in to manage your hangouts");
    expect(html).toContain("<h1>My Hangouts</h1>");
    expect(html).toContain('href="/events?tab=my"');
  });

  it("should describe the private view to agents instead of an empty public feed", () => {
    expect(whatsOnToMarkdown({ tab: "my" })).toContain("# My Hangouts");
    expect(whatsOnToMarkdown({ tab: "my" })).toContain("(/events?tab=my)");
  });
});
