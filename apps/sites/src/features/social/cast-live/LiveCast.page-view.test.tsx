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

const page = vi.hoisted(() => vi.fn());
const cast = vi.hoisted(() => ({ getStreamInfo: vi.fn(), getStreamerToken: vi.fn(), getWatcherToken: vi.fn() }));
const auth = vi.hoisted(() => ({ identity: null as null | { signer: string } }));

vi.mock("@/core/lib/telemetry/track", async (original) => ({
  ...(await original<typeof import("@/core/lib/telemetry/track")>()),
  page,
  track: vi.fn(),
}));
vi.mock("@/data/lib/catalyst/cast", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/cast")>()),
  ...cast,
}));
vi.mock("./session", () => ({
  joinCast: vi.fn(),
  probeDevices: vi.fn(async () => ({ microphone: true, camera: true })),
  canDecode: () => true,
  onRoomLost: () => () => {},
}));
vi.mock("./CastStage", () => ({
  useCastTracks: () => [],
  useWatcherStage: () => ({ shown: null }),
  WatcherStage: () => null,
  StreamerStage: () => null,
}));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: auth.identity }) }));
vi.mock("@/features/components/auth/signin-store", () => ({ openSignIn: vi.fn() }));
vi.mock("@/features/components/chrome/SitesLayout", () => ({ default: ({ children }: { children?: ReactNode }) => <>{children}</> }));

import { joinCast, type CastSession } from "./session";
import LiveCastStream from "./LiveCastStream";
import LiveCastWatch from "./LiveCastWatch";

const TRACK_CTX = { sid: "s", story: "cast/stream", variant: "live" } as never;
const STREAMER = "Streaming on Decentraland Cast";
const WATCHER = "Watching on Decentraland Cast";

function renderAt(path: string, pattern: string, element: ReactNode) {
  // Under a root route with a session, as in the app, so the page views go out.
  const router = createMemoryRouter(
    [{ id: "root", path: "/", loader: () => ({ sid: "s" }), HydrateFallback: () => null, children: [{ path: pattern, element }] }],
    { initialEntries: [path] },
  );
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
}

const names = () => page.mock.calls.map(([name]) => name);

beforeEach(() => {
  auth.identity = null;
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("the Cast speaker page", () => {
  it("sends the base view, then one view naming the stream's place once it is known", async () => {
    cast.getStreamInfo.mockResolvedValue({ placeName: "Genesis Plaza", placeId: "p1", location: "0,0", isWorld: false });
    renderAt("/cast/s/key", "/cast/s/:token", <LiveCastStream trackCtx={TRACK_CTX} streamingKey="key" />);
    await waitFor(() => expect(names()).toEqual([STREAMER, `Genesis Plaza — ${STREAMER}`]));
    expect(page.mock.calls[1]?.[1]).toEqual({ placeName: "Genesis Plaza", location: "0,0", isWorld: false });
    await screen.findByRole("button", { name: "JOIN NOW" });
    expect(page).toHaveBeenCalledTimes(2);
  });

  it("sends only the base view when the stream key cannot be read", async () => {
    cast.getStreamInfo.mockRejectedValue(new Error("expired"));
    renderAt("/cast/s/key", "/cast/s/:token", <LiveCastStream trackCtx={TRACK_CTX} streamingKey="key" />);
    await waitFor(() => expect(cast.getStreamInfo).toHaveBeenCalled());
    await waitFor(() => expect(names()).toEqual([STREAMER]));
  });
});

describe("the Cast watcher page", () => {
  const session = {
    subscribe: () => () => {},
    disconnected: () => false,
    canPlaybackAudio: () => true,
    identity: () => "watch:room:uuid",
    participantCount: () => 1,
    reconnecting: () => null,
    undecodable: () => null,
    onAir: () => false,
    videoOnAir: () => false,
    disconnect: async () => {},
  } as unknown as CastSession;

  it("sends the base view, then one view naming the place once the viewer joins", async () => {
    auth.identity = { signer: "0x00000000000000000000000000000000000abcd1" };
    cast.getWatcherToken.mockResolvedValue({ url: "wss://lk", token: "jwt", roomId: "room", placeName: "Genesis Plaza" });
    vi.mocked(joinCast).mockResolvedValue(session);
    renderAt("/cast/w/0,0", "/cast/w/:location", <LiveCastWatch trackCtx={TRACK_CTX} location="0,0" />);
    await waitFor(() => expect(names()).toEqual([WATCHER]));

    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    await waitFor(() => expect(names()).toEqual([WATCHER, `Genesis Plaza — ${WATCHER}`]));
    expect(page.mock.calls[1]?.[1]).toEqual({ placeName: "Genesis Plaza", location: "0,0", isWorld: false });
  });

  it("sends only the base view when the gatekeeper refuses the viewer", async () => {
    auth.identity = { signer: "0x00000000000000000000000000000000000abcd1" };
    cast.getWatcherToken.mockRejectedValue(new Error("refused"));
    renderAt("/cast/w/0,0", "/cast/w/:location", <LiveCastWatch trackCtx={TRACK_CTX} location="0,0" />);
    await waitFor(() => expect(names()).toEqual([WATCHER]));

    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    await waitFor(() => expect(cast.getWatcherToken).toHaveBeenCalled());
    await screen.findByText("Could not join the cast");
    expect(names()).toEqual([WATCHER]);
  });
});
