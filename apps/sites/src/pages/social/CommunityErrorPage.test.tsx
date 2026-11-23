// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { createMemoryRouter, data, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const net = vi.hoisted(() => ({ fetchCommunity: vi.fn() }));
vi.mock("@/data/lib/catalyst/social/community-detail", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/community-detail")>()),
  fetchCommunity: net.fetchCommunity,
}));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: null }) }));
vi.mock("@/features/components/chrome/SitesLayout", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("@/features/events/useCreatorProfiles", () => ({ useCreatorProfiles: () => ({}) }));
vi.mock("@/features/events/useEventReminders", () => ({
  useEventReminders: () => ({ get: () => ({ active: false, pending: false, toggle: () => {} }) }),
}));

import { CatalystError } from "@/data/lib/catalyst/client";
import { CommunityNotFoundError } from "@/data/lib/catalyst/social/community-detail";
import { communityKeys } from "@/data/lib/community/queries";

import { CommunityErrorPage } from "./CommunityErrorPage";
import { CommunityPage } from "./CommunityPage";

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
let client: QueryClient;

function show(loader?: () => unknown, element: ReactNode = <CommunityPage id={ID} now={0} />) {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(
    [
      { path: "/social/communities/:id", loader, element, ErrorBoundary: CommunityErrorPage },
      { path: "/elsewhere", element: <p>Elsewhere</p> },
    ],
    { initialEntries: [`/social/communities/${ID}`] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return router;
}

beforeEach(() => net.fetchCommunity.mockReset());
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.restoreAllMocks();
});

describe("CommunityErrorPage", () => {
  describe("when a document request's loader throws a 404", () => {
    beforeEach(() => {
      show(() => {
        throw data({ reason: "not-found" }, { status: 404 });
      });
    });

    it("should render the community as not found", async () => {
      expect(await screen.findByRole("heading", { name: "Community not found" })).toBeTruthy();
    });

    it("should offer no retry", async () => {
      await screen.findByRole("heading", { name: "Community not found" });
      expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
    });
  });

  describe("when a document request's loader throws a 503", () => {
    beforeEach(() => {
      show(() => {
        throw data({ reason: "unavailable" }, { status: 503 });
      });
    });

    it("should render the community as unavailable", async () => {
      expect(await screen.findByRole("heading", { name: "Community unavailable" })).toBeTruthy();
    });

    it("should offer a retry", async () => {
      await screen.findByRole("heading", { name: "Community unavailable" });
      expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
    });
  });

  describe("when a client navigation reads an unknown community", () => {
    beforeEach(() => {
      net.fetchCommunity.mockRejectedValueOnce(new CommunityNotFoundError(ID));
      show();
    });

    it("should render the community as not found", async () => {
      expect(await screen.findByRole("heading", { name: "Community not found" })).toBeTruthy();
    });
  });

  describe("when a client navigation's read fails", () => {
    describe("and the visitor retries", () => {
      let community: {
        id: string;
        name: string;
        description: null;
        ownerAddress: string;
        ownerName: null;
        privacy: "private";
        membersCount: number;
        thumbnailUrl: null;
        role: null;
      };

      beforeEach(async () => {
        community = {
          id: ID,
          name: "Decentraland Foundation",
          description: null,
          ownerAddress: "0xbcac4dafb7e215f2f6cb3312af6d5e4f9d9e7eda",
          ownerName: null,
          privacy: "private",
          membersCount: 1,
          thumbnailUrl: null,
          role: null,
        };
        net.fetchCommunity.mockRejectedValueOnce(new CatalystError("502", "/v1/communities", 502)).mockResolvedValueOnce(community);
        show();
        await userEvent.click(await screen.findByRole("button", { name: "Try again" }));
      });

      it("should render the community", async () => {
        expect(await screen.findByRole("heading", { level: 1, name: "Decentraland Foundation" })).toBeTruthy();
      });
    });

    describe("and the visitor leaves", () => {
      beforeEach(async () => {
        net.fetchCommunity.mockRejectedValueOnce(new CatalystError("502", "/v1/communities", 502));
        const router = show();
        await screen.findByRole("heading", { name: "Community unavailable" });
        await router.navigate("/elsewhere");
      });

      it("should release the failed read", async () => {
        await waitFor(() => expect(client.getQueryState(communityKeys.detail(ID))?.status).toBe("pending"));
      });
    });
  });

  describe("when the page throws any other error", () => {
    beforeEach(() => {
      const Root = () => {
        throw new Error("bug");
      };
      client = new QueryClient();
      const router = createMemoryRouter(
        [
          {
            path: "/",
            ErrorBoundary: () => <p>Root boundary</p>,
            children: [{ path: "social/communities/:id", element: <Root />, ErrorBoundary: CommunityErrorPage }],
          },
        ],
        { initialEntries: [`/social/communities/${ID}`] },
      );
      vi.spyOn(console, "error").mockImplementation(() => {});
      render(
        <QueryClientProvider client={client}>
          <RouterProvider router={router} />
        </QueryClientProvider>,
      );
    });

    it("should pass it up to the root boundary", async () => {
      expect(await screen.findByText("Root boundary")).toBeTruthy();
    });
  });
});
