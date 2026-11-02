import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AuthIdentity } from "@/data/lib/auth/types";
import WhatsOn, { loader } from "./whats-on";
import { fetchEvents } from "@/data/lib/catalyst/places/events";
import { pageLoader } from "@/core/lib/telemetry/page-loader";

const auth = vi.hoisted(() => ({ identity: null as AuthIdentity | null, address: null as string | null, isConnected: false }));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => auth }));
vi.mock("@/data/lib/catalyst/places/events", async (original) => ({ ...await original<typeof import("@/data/lib/catalyst/places/events")>(), fetchEvents: vi.fn() }));
vi.mock("@/core/lib/telemetry/page-loader", () => ({ pageLoader: vi.fn() }));

describe("when the main What's On route opens My Hangouts", () => {
  let renderRoute: () => string;
  let html: string;
  beforeEach(() => {
    auth.identity = null; auth.address = null; auth.isConnected = false;
    renderRoute = () => {
      const router = createMemoryRouter([{ id: "whats-on", path: "/whats-on", Component: WhatsOn as never }], {
        initialEntries: ["/whats-on?tab=my"], hydrationData: { loaderData: { "whats-on": {} } },
      });
      return renderToString(<RouterProvider router={router} />).replace(/<!--.*?-->/g, "");
    };
  });
  afterEach(() => vi.restoreAllMocks());

  it("should render the real My Hangouts page and sign-in action", () => {
    html = renderRoute();
    expect(html).toContain("Sign in to manage your hangouts");
    expect(html).toContain("<h1>My Hangouts</h1>");
  });

  it("should expose a reachable create route without showing privileged admin navigation", () => {
    html = renderRoute();
    expect(html).toContain('href="/events/new-event"');
    expect(html).not.toContain('href="/events/admin/users"');
  });

  it("should skip public feed and feed telemetry work for My Hangouts", async () => {
    await loader({ request: new Request("https://example.org/whats-on?tab=my") } as never);
    expect(fetchEvents).not.toHaveBeenCalled();
    expect(pageLoader).not.toHaveBeenCalled();
  });
});
