// @vitest-environment jsdom
import "@/features/locale/test-setup";
import { useState, type ReactNode } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRoutesStub } from "react-router";
import { describe, expect, it, vi } from "vitest";

import { ChromeAuthContext, type ChromeAuth } from "@ui/web/frames/chrome-auth";

const ACCOUNT = "0x1111111111111111111111111111111111112222";

// The page's own data and download plumbing are out of scope here; only the session wiring is.
vi.mock("@/features/download/useDownloadPage", () => ({
  useDownloadPage: () => ({
    ctas: {},
    jumpIn: vi.fn(),
    launcherMissing: false,
    dismissLauncherMissing: vi.fn(),
  }),
}));
vi.mock("@/features/download/action-props", () => ({
  downloadActionsProps: () => ({}),
  storeBadgeProps: () => undefined,
}));
vi.mock("@/data/lib/auth/use-profile-name", () => ({
  useProfileIdentity: () => ({ name: "", avatarUrl: "" }),
}));
vi.mock("@ui/wearable-preview/WearablePreview", () => ({
  default: () => null,
}));
vi.mock("@/data/lib/auth/index", () => ({
  useAuth: () => ({
    isConnected: true,
    address: ACCOUNT,
    fetch: async () => new Response("[]"),
    disconnect: vi.fn(),
  }),
}));
vi.mock("@ui/shop/credits/hooks", () => ({
  useBalance: () => ({ data: undefined, isError: false }),
}));
vi.mock("../shop/host", () => ({ installSitesShopClient: () => undefined }));

import DownloadRoute from "./download";

const loaderData = {
  sid: "sid-1",
  messages: { locale: "en" as const, messages: {}, fallback: {} },
  downloadsLabel: "+400K",
  platform: {
    kind: "desktop" as const,
    os: "macos" as const,
    arch: "unknown" as const,
  },
  // From the session cookie (or ?user=): the account the page previews.
  account: ACCOUNT,
};

/** The shared chrome auth as ChromeAuthBridge provides it, with a sign out that ends the session. */
function Session({
  initial,
  children,
}: {
  initial: boolean;
  children: ReactNode;
}) {
  const [signedIn, setSignedIn] = useState(initial);
  const value: ChromeAuth = {
    signedIn,
    account: signedIn ? ACCOUNT : "",
    mana: "",
    name: "Alice",
    committee: false,
    onSignIn: vi.fn(),
    onSignOut: () => setSignedIn(false),
  };
  return (
    <ChromeAuthContext.Provider value={value}>
      {children}
    </ChromeAuthContext.Provider>
  );
}

// The route module as the app mounts it: its component reads the loader's data from a data router.
function renderRoute(signedIn: boolean) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const Stub = createRoutesStub([
    { path: "/download", Component: DownloadRoute, loader: () => loaderData },
  ]);
  render(
    <QueryClientProvider client={queryClient}>
      <Session initial={signedIn}>
        <Stub initialEntries={["/download"]} />
      </Session>
    </QueryClientProvider>,
  );
}

describe("/download session wiring", () => {
  it("swaps the signed-in navbar for the signed-out Sign In header on logout, without a reload", async () => {
    renderRoute(true);

    expect(
      await screen.findByRole("button", {
        name: "Profile",
      }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", {
        name: "Sign In",
      }),
    ).not.toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", {
        name: "Profile",
      }),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "component.landing.navbar.logout" }),
    );

    expect(
      screen.queryByRole("button", {
        name: "Profile",
      }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Sign In" })).toBeInTheDocument();
  });

  it("shows the Sign In header when the cookie named an account but the live session is gone", async () => {
    renderRoute(false);

    expect(
      await screen.findByRole("button", {
        name: "Sign In",
      }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", {
        name: "Profile",
      }),
    ).not.toBeInTheDocument();
  });
});
