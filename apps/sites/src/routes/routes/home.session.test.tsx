// @vitest-environment jsdom
import "@/features/locale/test-setup";
import { useState, type ReactNode } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRoutesStub } from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ChromeAuthContext, type ChromeAuth } from "@ui/web/frames/chrome-auth";

const ACCOUNT = "0x1111111111111111111111111111111111112222";

// jsdom has no media queries; the hero and clips only read them for motion and layout.
window.matchMedia = ((media: string) => ({
  matches: false,
  media,
  addEventListener: () => undefined,
  removeEventListener: () => undefined,
})) as unknown as typeof window.matchMedia;

// The page's own data and download plumbing are out of scope here; only the session wiring is.
vi.mock("@/features/download/useDownloadCtas", () => ({
  useDownloadCtas: () => ({ request: { flags: {}, campaign: {} } }),
}));
const track = vi.fn();
vi.mock("@/core/lib/telemetry/track", () => ({
  track: (...args: unknown[]) => track(...args),
}));
const openSignIn = vi.fn();
vi.mock("@/features/components/auth/signin-store", () => ({
  openSignIn: (...args: unknown[]) => openSignIn(...args),
}));
vi.mock("@/features/download/launch", () => ({
  launchDesktopApp: async () => false,
}));
vi.mock("@/features/download/action-props", () => ({
  downloadActionsProps: () => ({}),
  storeBadgeProps: () => undefined,
}));
vi.mock("@/data/lib/auth/use-profile-name", () => ({
  useProfileIdentity: () => ({ name: "", avatarUrl: "" }),
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

import HomeRoute from "./_index";
import navbarStyles from "@dcl/one-ui/apps/sites/chrome/SitesNavbar.module.css";

const loaderData = {
  sid: "sid-1",
  messages: { locale: "en" as const, messages: {}, fallback: {} },
  whatsOn: [],
  downloadsLabel: "+400K",
  platform: {
    kind: "desktop" as const,
    os: "macos" as const,
    arch: "unknown" as const,
  },
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
    { path: "/", Component: HomeRoute, loader: () => loaderData },
  ]);
  render(
    <QueryClientProvider client={queryClient}>
      <Session initial={signedIn}>
        <Stub initialEntries={["/"]} />
      </Session>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  Object.defineProperty(window, "innerWidth", {
    value: 1440,
    configurable: true,
  });
  track.mockClear();
  openSignIn.mockClear();
});

describe("home signed-out header clicks, as source's minimal navbar reports them", () => {
  const SESSION = { sid: "sid-1", story: "discover" };
  const navbarClick = (props: Record<string, unknown>) =>
    expect.objectContaining({
      place: "Landing Navbar",
      event: "click",
      track_deferred: false,
      ...props,
    });
  const clicks = () =>
    track.mock.calls.filter(
      ([event, props]) => event === "Click" && props.place === "Landing Navbar",
    );

  it("sends sign_in, then opens sign in", async () => {
    renderRoute(false);
    await userEvent.click(
      await screen.findByRole("button", {
        name: "Sign In",
      }),
    );

    expect(clicks()).toEqual([
      ["Click", navbarClick({ action: "sign_in" }), SESSION],
    ]);
    expect(clicks()[0]![1]).not.toHaveProperty("section");
    expect(openSignIn).toHaveBeenCalledTimes(1);
  });

  it("sends the logo as action logo in section home", async () => {
    renderRoute(false);
    const logo = await screen.findByRole("link", {
      name: "Decentraland",
    });
    logo.addEventListener("click", (event) => event.preventDefault());
    await userEvent.click(logo);

    expect(clicks()).toEqual([
      [
        "Click",
        navbarClick({ section: "home", action: "logo", href: "/" }),
        SESSION,
      ],
    ]);
  });
});

describe("home session wiring", () => {
  it("swaps the signed-in navbar for the signed-out landing header on logout, without a reload", async () => {
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
    expect(screen.getByRole("banner")).toHaveClass(
      navbarStyles.generatedOverlay,
    );
    expect(screen.getByRole("navigation", { name: "Main" })).toHaveClass(
      navbarStyles.generatedTransparent,
    );

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

  it("shows the landing header when the session cookie is stale and the live session is gone", async () => {
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
