import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AuthIdentity } from "@/data/lib/auth/types";
import SubmitHangoutRoute from "./events_.new-event";

const auth = vi.hoisted(() => ({ identity: null as AuthIdentity | null, address: null as string | null, isConnected: false }));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => auth }));

describe("when the actual hangout route renders the details step", () => {
  let renderRoute: () => string;
  let html: string;

  beforeEach(() => {
    auth.identity = { signer: "0x1111111111111111111111111111111111111111", ephemeral: { address: "0x2222222222222222222222222222222222222222", privateKey: "0x01" }, expiration: "2099-01-01T00:00:00Z", authChain: [] };
    auth.address = auth.identity.signer;
    auth.isConnected = true;
    renderRoute = () => {
      const router = createMemoryRouter([{ id: "hangout", path: "/events/new-event", Component: SubmitHangoutRoute as never }], {
        initialEntries: ["/events/new-event?step=details"],
        hydrationData: { loaderData: { hangout: { sid: "test-session", mode: "create", eventId: null, communityId: "", categories: [], categoriesError: false, assignment: { variant: "wizard" } } } },
      });
      return renderToString(<QueryClientProvider client={new QueryClient()}><RouterProvider router={router} /></QueryClientProvider>);
    };
  });

  afterEach(() => vi.restoreAllMocks());

  it("should render one connected name field without the inert duplicate form", () => {
    html = renderRoute();
    expect(html.match(/Hangout Name/g)).toHaveLength(1);
  });

  it("should omit the sample event and simulated success copy", () => {
    html = renderRoute();
    expect(html).not.toMatch(/Neon Nights|simulated|Music Lovers DAO/);
  });

  describe("and the visitor is signed out", () => {
    beforeEach(() => { auth.identity = null; auth.address = null; auth.isConnected = false; });

    it("should render only the loading page while sign-in opens, with no form and no sign-in button", () => {
      html = renderRoute();
      expect(html).toMatch(/<p[^>]*role="status"[^>]*><span role="progressbar" aria-label="Loading your hangout…"/);
      expect(html).not.toContain("Sign In to continue");
      expect(html).not.toContain("Hangout Name");
    });
  });
});
