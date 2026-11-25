// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

const room = vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
  return { lost: false, removed: false, listeners: new Set<() => void>() };
});
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("../../components/chrome/SitesLayout", () => ({ default: ({ children }: { children: import("react").ReactNode }) => <>{children}</> }));
vi.mock("@/data/lib/catalyst/cast", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/cast")>()),
  getStreamInfo: async () => ({ placeName: "Test Plaza", placeId: "p1", location: "1,2", isWorld: false }),
  getStreamerToken: async () => ({ url: "wss://livekit.test", token: "jwt", roomId: "room-1", identity: "Ada" }),
}));
vi.mock("./session", async (original) => ({
  ...(await original<typeof import("./session")>()),
  probeDevices: async () => ({ microphone: true, camera: true }),
  joinCast: async () => ({ tracks: () => [], participantCount: () => 1, reconnecting: () => null, isPublishing: () => false, canPlaybackAudio: () => true, disconnected: () => room.lost, removed: () => room.removed, subscribe: (l: () => void) => (room.listeners.add(l), () => room.listeners.delete(l)), disconnect: async () => {} }),
}));

import LiveCastStream from "./LiveCastStream";

/** The streamer page after a click on JOIN NOW, live in its room. */
async function live() {
  const Page = () => <LiveCastStream trackCtx={{ sid: "sid", story: "cast/stream", variant: "live" }} streamingKey="stream-key" />;
  render(<RouterProvider router={createMemoryRouter([{ path: "/", Component: Page }])} />);
  fireEvent.click(await screen.findByRole("button", { name: "JOIN NOW" }));
  await screen.findByRole("button", { name: "Leave stream" });
}

/** The room goes away. */
function end(removed: boolean) {
  Object.assign(room, { lost: true, removed });
  act(() => room.listeners.forEach((l) => l()));
}

afterEach(() => {
  cleanup();
  Object.assign(room, { lost: false, removed: false });
  room.listeners.clear();
});

describe("the Cast streamer page", () => {
  it("tells a streamer that the server removed so, with no Rejoin", async () => {
    await live();
    end(true);
    expect(screen.getByRole("alertdialog", { name: "You were removed from this Cast" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Rejoin" })).toBeNull();
  });

  it("ends a room that was gone before the page listened to it", async () => {
    room.lost = true;
    const Page = () => <LiveCastStream trackCtx={{ sid: "sid", story: "cast/stream", variant: "live" }} streamingKey="stream-key" />;
    render(<RouterProvider router={createMemoryRouter([{ path: "/", Component: Page }])} />);
    fireEvent.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    expect(await screen.findByRole("alertdialog", { name: "Your cast was disconnected" })).toBeTruthy();
  });

  it("offers Rejoin to a streamer whose connection was lost", async () => {
    await live();
    end(false);
    expect(screen.getByRole("alertdialog", { name: "Your cast was disconnected" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Rejoin" })).toBeTruthy();
  });
});
