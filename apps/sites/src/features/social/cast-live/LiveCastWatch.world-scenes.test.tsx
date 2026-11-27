// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

const cast = vi.hoisted(() => ({ getWatcherToken: vi.fn() }));
const SIGNER = "0x00000000000000000000000000000000000abcd1";
const auth = vi.hoisted(() => ({ identity: null as null | { signer: string } }));
const openSignIn = vi.hoisted(() => vi.fn());
vi.mock("@/core/lib/telemetry/track", async (original) => ({
  ...(await original<typeof import("@/core/lib/telemetry/track")>()),
  page: vi.fn(),
  track: vi.fn(),
}));
vi.mock("@/data/lib/catalyst/cast", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/cast")>()),
  ...cast,
}));
vi.mock("@/features/components/auth/signin-store", () => ({ openSignIn }));
vi.mock("./session", () => ({ joinCast: vi.fn() }));
vi.mock("./CastStage", () => ({ useCastTracks: () => [], WatcherStage: () => null }));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: auth.identity }) }));
vi.mock("@/features/components/chrome/SitesLayout", () => ({ default: ({ children }: { children?: ReactNode }) => <>{children}</> }));

import { joinCast, type CastSession } from "./session";
import LiveCastWatch from "./LiveCastWatch";

const TRACK_CTX = { sid: "s", story: "cast/watch", variant: "live" } as never;
const WORLD = "pixelpixie.dcl.eth";
const SCENES = [{ base: "0,0", title: "Main stage" }, { base: "4,2", title: "Second stage" }];

function renderAt(location: string, scenes: typeof SCENES | null, parcel?: string) {
  const router = createMemoryRouter(
    [{ id: "root", path: "/", loader: () => ({ sid: "s" }), HydrateFallback: () => null, children: [{
      path: "/cast/w/:location",
      element: <LiveCastWatch trackCtx={TRACK_CTX} location={location} scenes={scenes} parcel={parcel} />,
    }] }],
    { initialEntries: [`/cast/w/${location}`] },
  );
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  auth.identity = { signer: SIGNER };
  cast.getWatcherToken.mockResolvedValue({ url: "wss://lk", token: "jwt", roomId: "room" });
  vi.mocked(joinCast).mockResolvedValue({
    subscribe: () => () => {},
    disconnected: () => false,
    canPlaybackAudio: () => true,
    participantCount: () => 1,
    disconnect: async () => {},
  } as unknown as CastSession);
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("a world watcher link without a parcel", () => {
  it("asks for a scene before joining a multi-scene world", async () => {
    renderAt(WORLD, SCENES);
    await screen.findByText("Choose a scene to watch");
    expect(screen.getByRole("button", { name: "Main stage 0,0" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "JOIN NOW" })).toBeNull();
    expect(cast.getWatcherToken).not.toHaveBeenCalled();

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Second stage 4,2" }));
    expect(cast.getWatcherToken).not.toHaveBeenCalled();
    await user.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    await waitFor(() => expect(cast.getWatcherToken).toHaveBeenCalledTimes(1));
    expect(cast.getWatcherToken).toHaveBeenCalledWith(auth.identity,
      expect.objectContaining({ location: WORLD, parcel: "4,2" }), expect.anything());
  });

  it("automatically uses the only scene's parcel", async () => {
    renderAt(WORLD, SCENES.slice(0, 1));
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    expect(screen.queryByText("Choose a scene to watch")).toBeNull();
    expect(cast.getWatcherToken).toHaveBeenCalledWith(auth.identity,
      expect.objectContaining({ location: WORLD, parcel: "0,0" }), expect.anything());
  });

  it.each([[WORLD, null], [WORLD, []], ["0,0", null]] as const)(
    "keeps the parcel absent for %s with scenes %j", async (location, scenes) => {
      renderAt(location, scenes as typeof SCENES | null);
      const user = userEvent.setup();
      await user.click(await screen.findByRole("button", { name: "JOIN NOW" }));
      expect(cast.getWatcherToken).toHaveBeenCalledWith(auth.identity,
        expect.objectContaining({ location, parcel: undefined }), expect.anything());
    },
  );

  it("uses an explicit parcel ahead of the scene list", async () => {
    renderAt(WORLD, SCENES, "9,9");
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    expect(screen.queryByText("Choose a scene to watch")).toBeNull();
    expect(cast.getWatcherToken).toHaveBeenCalledWith(auth.identity,
      expect.objectContaining({ location: WORLD, parcel: "9,9" }), expect.anything());
  });
});


describe("watcher accounts", () => {
  it("joins signed out with a guest instead of asking for an account", async () => {
    auth.identity = null;
    renderAt("0,0", null);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    await waitFor(() => expect(cast.getWatcherToken).toHaveBeenCalledTimes(1));
    const [identity, body] = cast.getWatcherToken.mock.calls[0];
    expect(identity.signer).toMatch(/^0x[0-9a-f]{40}$/);
    expect(body.displayName).toBe(`Guest#${identity.signer.slice(-4)}`);
    expect(openSignIn).not.toHaveBeenCalled();
    expect(screen.queryByText("Sign in to watch")).toBeNull();
  });

  it("uses the signed-in account when one is available", async () => {
    renderAt("0,0", null);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    expect(cast.getWatcherToken.mock.calls[0][0]).toBe(auth.identity);
  });
});
