import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AuthIdentity } from "@/data/lib/auth/types";
import SubmitHangoutRoute from "./landings.submit-hangout";

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
      const router = createMemoryRouter([{ id: "hangout", path: "/landings/submit-hangout", Component: SubmitHangoutRoute as never }], {
        initialEntries: ["/landings/submit-hangout?step=details"],
        hydrationData: { loaderData: { hangout: { sid: "test-session", mode: "create", eventId: null, communityId: "", categories: [], categoriesError: false, assignment: { variant: "wizard" } } } },
      });
      return renderToString(<RouterProvider router={router} />);
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

    it("should show the real sign-in action instead of editable hangout fields", () => {
      html = renderRoute();
      expect(html).toContain("Sign In to continue");
      expect(html).not.toContain("Hangout Name");
    });
  });
});
