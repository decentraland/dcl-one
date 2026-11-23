// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { createMemoryRouter, Outlet, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { track } from "@/core/lib/telemetry/track";

import HelpRoute from "./help";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

/** The document's readiness (its first page view), driven by the test. */
const readiness = vi.hoisted(() => ({ ready: true, pending: [] as Array<() => void> }));

vi.mock("@/core/lib/telemetry/track", () => ({
  track: vi.fn(),
  legacyAnalyticsReady: () => readiness.ready,
  whenLegacyAnalyticsReady: (fire: () => void) => {
    if (readiness.ready) {
      fire();
      return () => {};
    }
    readiness.pending.push(fire);
    return () => {
      readiness.pending = readiness.pending.filter((queued) => queued !== fire);
    };
  },
}));
vi.mock("@/features/components/chrome/SitesLayout", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));

const EMAIL = "creatorfund@decentraland.org";

function renderHelp() {
  const router = createMemoryRouter(
    [{ id: "root", path: "/", Component: Outlet, children: [{ id: "help", path: "help", Component: HelpRoute as never }] }],
    {
      initialEntries: ["/help?utm_source=newsletter"],
      hydrationData: {
        loaderData: {
          root: { sid: "sid-root", analyticsUserId: "0xabc" },
          help: { services: [], statusPageUrl: "https://status.example/", intercomAppId: "", sid: "sid-help", variant: "config_driven" },
        },
      },
    },
  );
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
}

/** The legacy `Click`s sent, less their audit timestamps (asserted to be numbers). */
function legacyClicks() {
  return vi
    .mocked(track)
    .mock.calls.filter(([event]) => event === "Click")
    .map(([, props, ctx]) => {
      const { track_called_at, track_delivered_at, ...rest } = props as Record<string, unknown>;
      expect(typeof track_called_at).toBe("number");
      expect(typeof track_delivered_at).toBe("number");
      return [rest, ctx];
    });
}

const CTX = { sid: "sid-help", userId: "0xabc", legacy: true };

describe("/help Creator Fund tab, legacy Click (decentraland/sites CreatorFundFAQSection rows 72-73)", () => {
  let user: ReturnType<typeof userEvent.setup>;

  beforeEach(async () => {
    readiness.ready = true;
    readiness.pending = [];
    vi.stubGlobal("fetch", vi.fn(async () => new Response(null, { status: 200 })));
    // The page URL carries a campaign, which decentraland.org put first in every Click.
    window.history.replaceState(null, "", "/help?utm_source=newsletter");
    user = userEvent.setup();
    renderHelp();
    await user.click(screen.getByRole("tab", { name: "CREATOR FUND" }));
  });

  afterEach(() => {
    cleanup();
    vi.resetAllMocks();
    vi.unstubAllGlobals();
    window.history.replaceState(null, "", "/");
  });

  describe("when a question is opened and then closed", () => {
    beforeEach(async () => {
      const question = screen.getByRole("button", { name: "How Do I Join?" });
      await user.click(question);
      await user.click(question);
    });

    it("should send one Click, for the opening, with the question's tracking name", () => {
      expect(legacyClicks()).toEqual([
        [
          { utm_source: "newsletter", place: "Creator Fund", section: "help-center", button: "How Do I Join", track_deferred: false },
          CTX,
        ],
      ]);
    });

    it("should keep sending the site's own event too", () => {
      expect(vi.mocked(track).mock.calls.filter(([event]) => event === "lp_help_creator_fund_clicked")).toHaveLength(1);
    });
  });

  describe("when the contact address is clicked", () => {
    beforeEach(async () => {
      const contact = screen.getByRole("link", { name: EMAIL });
      contact.addEventListener("click", (event) => event.preventDefault());
      await user.click(contact);
    });

    it("should send the Email Creator Fund Click", () => {
      expect(legacyClicks()).toEqual([
        [
          {
            utm_source: "newsletter",
            place: "Creator Fund",
            section: "help-center",
            button: "Email Creator Fund",
            track_deferred: false,
          },
          CTX,
        ],
      ]);
    });
  });

  describe("when a question is opened before the document's analytics is ready", () => {
    beforeEach(async () => {
      readiness.ready = false;
      await user.click(screen.getByRole("button", { name: "Do Worlds Count?" }));
    });

    it("should send nothing until then", () => {
      expect(legacyClicks()).toEqual([]);
    });

    it("should send the Click, marked deferred, once it is ready", () => {
      readiness.ready = true;
      for (const fire of readiness.pending.splice(0)) fire();
      expect(legacyClicks()).toEqual([
        [
          { utm_source: "newsletter", place: "Creator Fund", section: "help-center", button: "Do Worlds Count", track_deferred: true },
          CTX,
        ],
      ]);
    });
  });

  describe("when the document's analytics never becomes ready (a bot, an exempt session, no write key)", () => {
    beforeEach(async () => {
      readiness.ready = false;
      await user.click(screen.getByRole("button", { name: "Do Worlds Count?" }));
      cleanup();
    });

    it("should drop the Click when the page goes away", () => {
      expect(readiness.pending).toEqual([]);
      expect(legacyClicks()).toEqual([]);
    });
  });
});
