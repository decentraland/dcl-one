// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

const facts = vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
  return { identity: null as { signer: string } | null, openSignIn: vi.fn(), lost: false, removed: false, reconnecting: null as "signal" | "all" | null, listeners: new Set<() => void>(), tracks: [] as object[], codec: null as string | null, decodes: false, room: [] as string[], join: vi.fn(), disconnect: vi.fn(async () => {}) };
});
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: facts.identity }) }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("../../components/auth/signin-store", () => ({ openSignIn: facts.openSignIn }));
vi.mock("../../components/chrome/SitesLayout", () => ({ default: ({ children }: { children: import("react").ReactNode }) => <>{children}</> }));
vi.mock("@/data/lib/catalyst/cast", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/cast")>()),
  getWatcherToken: async () => ({ url: "wss://livekit.test", token: "jwt", roomId: "room-1", identity: "Ada", placeName: "Test Plaza" }),
}));
vi.mock("./session", async (original) => ({
  ...(await original<typeof import("./session")>()),
  canDecode: () => facts.decodes,
  // Each join has its own identity; `facts.room` is who else the server lists.
  joinCast: facts.join.mockImplementation(async () => {
    const identity = `watch:room:${facts.join.mock.calls.length}`;
    const participantCount = (own?: ReadonlySet<string>) => 1 + facts.room.filter((id) => id !== identity && !own?.has(id)).length;
    return { identity: () => identity, participantCount, undecodable: () => facts.codec, tracks: () => facts.tracks, videoOnAir: () => facts.tracks.length > 0 || facts.codec !== null, onAir: () => true, reconnecting: () => facts.reconnecting, disconnected: () => facts.lost, removed: () => facts.removed, subscribe: (l: () => void) => (facts.listeners.add(l), () => facts.listeners.delete(l)), canPlaybackAudio: () => true, disconnect: facts.disconnect };
  }),
}));

import { useCastTracks } from "./CastStage";
import LiveCastWatch from "./LiveCastWatch";
import type { CastSession } from "./session";

const WORLD = "stage.dcl.eth";
const LOBBY = { name: `Decentraland Cast from ${WORLD}` };

function open() {
  const Page = () => <LiveCastWatch trackCtx={{ sid: "sid", story: "cast/watch", variant: "live" }} location={WORLD} />;
  render(<RouterProvider router={createMemoryRouter([{ path: "/", Component: Page }])} />);
}

/** A remote video of the streamer, as the session lists it. */
function video(source: "camera" | "screen_share", facts: { picture?: boolean; stalled?: boolean } = {}) {
  const track = { attach: vi.fn(), detach: vi.fn() };
  return { key: source, identity: "stream:place:uuid", source, muted: false, local: false, stalled: false, picture: true, track, ...facts };
}

/** How many tracks the page has of a session. A hook call in a test's own function fails the file's transform. */
function TrackCount({ session }: { session: CastSession }) {
  return <output>{useCastTracks(session).length}</output>;
}

/** The room now has these tracks. */
/** The room changes. The page's effects run first, so each of its listeners is there to hear it. */
async function air(...tracks: object[]) {
  facts.tracks = tracks;
  await act(async () => {});
  act(() => facts.listeners.forEach((l) => l()));
}

const shown = () => [...document.querySelectorAll("video")].map((v) => `${v.dataset.source}${v.style.opacity === "0" ? " (unseen)" : ""}`);

/** Holds the page's timers from now on. A call of `vi.use…` in a test's own function fails the file's transform. */
function hold() {
  vi.useFakeTimers();
}

/** The joined page loses its room; its timers are held from then on. */
async function lose() {
  await act(async () => {});
  hold();
  facts.lost = true;
  act(() => facts.listeners.forEach((l) => l()));
  facts.lost = false;
}

afterEach(() => {
  cleanup();
  facts.identity = null;
  facts.openSignIn.mockClear();
  Object.assign(facts, { lost: false, removed: false, reconnecting: null, tracks: [], codec: null, decodes: false, room: [] });
  facts.listeners.clear();
  facts.join.mockClear();
  facts.disconnect.mockClear();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("the Cast watcher page", () => {
  it("names the place the same way before the join and after LEAVE", async () => {
    facts.identity = { signer: "0x" + "ab".repeat(20) };
    open();
    expect(await screen.findByRole("heading", LOBBY)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "JOIN NOW" }));
    fireEvent.click(await screen.findByRole("button", { name: "Leave" }));
    expect(await screen.findByRole("heading", LOBBY)).toBeTruthy();
  });

  it("opens the sign-in for a visitor who is not signed in, and keeps the lobby with no notice", async () => {
    open();
    fireEvent.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    expect(await screen.findByRole("button", { name: "JOIN NOW" })).toBeTruthy();
    expect(facts.openSignIn).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("heading", LOBBY)).toBeTruthy();
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(facts.join).not.toHaveBeenCalled();
  });

  it("joins again by itself after a room that was gone before the page listened to it", async () => {
    facts.identity = { signer: "0x" + "ab".repeat(20) };
    facts.lost = true;
    open();
    fireEvent.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    expect(await screen.findByText("Reconnecting\u{2026} (1 of 3)")).toBeTruthy();
    expect(screen.queryByText("LIVE")).toBeNull();
  });

  it("keeps the camera until a screen share has its picture, then keeps the share, stalled, with the notice and no LIVE", async () => {
    facts.identity = { signer: "0x" + "ab".repeat(20) };
    const camera = video("camera");
    const share = video("screen_share", { picture: false });
    facts.tracks = [video("camera", { picture: false })];
    open();
    fireEvent.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    await screen.findByRole("button", { name: "Leave" });
    expect(screen.queryByText("LIVE")).toBeNull();
    expect(screen.queryByRole("status")).toBeNull();

    await air(camera, share);
    expect(shown()).toEqual(["camera", "screen_share (unseen)"]);
    expect(screen.getByText("LIVE")).toBeTruthy();

    const element = document.querySelectorAll("video")[1];
    await air(camera, { ...share, picture: true });
    expect(shown()).toEqual(["screen_share"]);
    expect(document.querySelector("video")).toBe(element);
    expect(share.track.attach).toHaveBeenCalledTimes(1);
    expect(screen.getByText("LIVE")).toBeTruthy();

    await air(camera, { ...share, picture: true, stalled: true });
    expect(shown()).toEqual(["screen_share"]);
    expect(screen.queryByText("LIVE")).toBeNull();
    expect(screen.getByRole("status").textContent).toContain("Reconnecting");
  });

  it("says that the browser cannot play the video, and joins again by itself when the browser has the codec, three times at most", async () => {
    facts.identity = { signer: "0x" + "ab".repeat(20) };
    open();
    fireEvent.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    await screen.findByText("LIVE");

    hold();
    facts.codec = "video/H264";
    await air();
    expect(screen.getByRole("status").textContent).toContain("This browser cannot play the video of this Cast yet");
    expect(screen.queryByText("LIVE")).toBeNull();
    await act(() => vi.advanceTimersByTimeAsync(60_000));
    expect(facts.join).toHaveBeenCalledTimes(1);
    facts.decodes = true;
    await act(() => vi.advanceTimersByTimeAsync(15_000));
    expect(facts.join).toHaveBeenCalledTimes(2);
    expect(facts.disconnect).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "Leave" })).toBeTruthy();
    await act(() => vi.advanceTimersByTimeAsync(120_000));
    expect(facts.join).toHaveBeenCalledTimes(4);

    facts.codec = null;
    await air(video("camera"));
    expect(screen.getByText("LIVE")).toBeTruthy();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("counts itself one time when it joins again for the codec and the new room still lists its first connection", async () => {
    facts.identity = { signer: "0x" + "ab".repeat(20) };
    facts.room = ["watch:room:other"];
    open();
    fireEvent.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    await screen.findByText("LIVE");
    const people = () => screen.getByRole("button", { name: "Participants" }).textContent;
    expect(people()).toBe("2");

    hold();
    Object.assign(facts, { codec: "video/H264", decodes: true, room: ["watch:room:other", "watch:room:1"] });
    await air();
    await act(() => vi.advanceTimersByTimeAsync(15_000));
    expect(facts.join).toHaveBeenCalledTimes(2);
    expect(facts.disconnect).toHaveBeenCalledTimes(1);
    expect(people()).toBe("2");
  });

  it("reads the room again when it starts to listen, for an event that came before", () => {
    const room = { tracks: [] as object[] };
    const early = () => ((room.tracks = [video("camera")]), () => {});
    render(<TrackCount session={{ tracks: () => room.tracks, subscribe: early } as unknown as CastSession} />);
    expect(screen.getByRole("status").textContent).toBe("1");
  });

  it("shows the end card of a watcher that the server removed, and does not join again by itself", async () => {
    facts.identity = { signer: "0x" + "ab".repeat(20) };
    open();
    fireEvent.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    await screen.findByText("LIVE");

    facts.removed = true;
    await lose();
    expect(screen.getByRole("alertdialog", { name: "You were removed from this Cast" })).toBeTruthy();
    expect(screen.queryByText(/Reconnecting/)).toBeNull();
    await act(() => vi.advanceTimersByTimeAsync(60_000));
    expect(facts.join).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "Join again" })).toBeTruthy();
  });

  it("says it reconnects, with no LIVE, while LiveKit restores the whole connection, and stays LIVE while it restores the signal", async () => {
    facts.identity = { signer: "0x" + "ab".repeat(20) };
    facts.tracks = [video("camera")];
    open();
    fireEvent.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    await screen.findByText("LIVE");

    facts.reconnecting = "signal";
    await air(video("camera"));
    expect(screen.getByText("LIVE")).toBeTruthy();
    expect(screen.queryByRole("status")).toBeNull();

    facts.reconnecting = "all";
    await air(video("camera"));
    await waitFor(() => expect(screen.queryByText("LIVE")).toBeNull());
    expect(screen.getByRole("status").textContent).toContain("Reconnecting");

    facts.reconnecting = null;
    await air(video("camera"));
    await waitFor(() => expect(screen.getByText("LIVE")).toBeTruthy());
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("gives a join by a click 45 s to answer, and an automatic join 10 s", async () => {
    const bound = vi.spyOn(AbortSignal, "timeout");
    facts.identity = { signer: "0x" + "ab".repeat(20) };
    open();
    fireEvent.click(await screen.findByRole("button", { name: "JOIN NOW" }));
    await screen.findByText("LIVE");
    expect(bound).toHaveBeenLastCalledWith(45_000);

    await lose();
    expect(screen.getByText("Reconnecting\u{2026} (1 of 3)")).toBeTruthy();
    await act(() => vi.advanceTimersByTimeAsync(2_000));
    expect(bound).toHaveBeenLastCalledWith(10_000);
  });
});
