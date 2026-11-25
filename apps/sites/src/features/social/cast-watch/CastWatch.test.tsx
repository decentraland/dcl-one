// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});
vi.mock("../../components/chrome/SitesLayout", () => ({ default: ({ children }: { children: import("react").ReactNode }) => <>{children}</> }));

import CastWatch, { type WatchLive, type WatchLoss } from "./CastWatch";

const WATCH = { status: "live", location: "0,0", isWorld: false, placeName: "Test Plaza", identity: "Guest#1" } as const;

/** The page after a click on JOIN NOW. */
async function joined(live: Partial<WatchLive>, onTrack = vi.fn()) {
  const all: WatchLive = {
    join: async () => ({ roomId: "room-1" }),
    leave: vi.fn(),
    stage: () => <video />,
    participantCount: 1,
    signal: "live",
    bindLost: () => () => {},
    audioBlocked: false,
    enableAudio: vi.fn(),
    ...live,
  };
  const Page = () => (
    <CastWatch trackCtx={{ sid: "sid", story: "cast/watch", variant: "live" }} watch={WATCH} live={all} onTrack={onTrack} />
  );
  const view = render(<RouterProvider router={createMemoryRouter([{ path: "/", Component: Page }])} />);
  fireEvent.click(await screen.findByRole("button", { name: "JOIN NOW" }));
  return view.container;
}

const people = () => screen.getByRole("button", { name: "Participants" }).textContent;

/** The joined page with its timers held; the answer loses its room. */
async function watching(live: Partial<WatchLive>) {
  let lost = (_why: WatchLoss) => {};
  await joined({ bindLost: (onLost) => ((lost = onLost), () => {}), ...live });
  await screen.findByRole("button", { name: "Leave" });
  vi.useFakeTimers();
  return (why: WatchLoss) => act(() => lost(why));
}

const pass = (ms: number) => act(() => vi.advanceTimersByTimeAsync(ms));
const tries = (n: number) => screen.getByText(`Reconnecting\u{2026} (${n} of 3)`);
const again = () => screen.queryAllByRole("button", { name: "Join again" });

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("the Cast watcher page", () => {
  it("shows LIVE and the stage while a track is on air", async () => {
    const container = await joined({ participantCount: 2 });
    expect(await screen.findByText("LIVE")).toBeTruthy();
    expect(container.querySelector("video")).not.toBeNull();
    expect(people()).toBe("2");
  });

  it("says no one is casting, without LIVE, in a room where nothing is on air", async () => {
    const container = await joined({ signal: "none" });
    expect(await screen.findByText("No one is currently casting to this link...")).toBeTruthy();
    expect(screen.queryByText("LIVE")).toBeNull();
    expect(container.querySelector(".cast-watch")?.getAttribute("data-state")).toBe("waiting");
    expect(people()).toBe("1");
  });

  it("shows no people number when no room was joined", async () => {
    await joined({
      participantCount: 0,
      join: async () => ({ refused: { kind: "waiting", title: "No stream", message: "Soon." } }),
    });
    await screen.findByText("Soon.");
    expect(people()).toBe("");
  });

  it("names a join that failed on the end card, and Join again joins", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    const join = vi
      .fn<WatchLive["join"]>(async () => ({ roomId: "room-1" }))
      .mockRejectedValueOnce(new Error("The Cast server did not answer in time"));
    await joined({ join });
    expect((await screen.findByRole("alertdialog")).textContent).toContain("The Cast server did not answer in time");
    fireEvent.click(again()[0]!);
    expect(await screen.findByText("LIVE")).toBeTruthy();
    expect(join).toHaveBeenCalledTimes(2);
  });

  it("says plain words on the end card of a join that failed, and keeps the error's text for telemetry and the console", async () => {
    const said = vi.spyOn(console, "warn").mockImplementation(() => {});
    const onTrack = vi.fn();
    const raw = "Catalyst returned 502 Bad Gateway";
    await joined({ join: async () => Promise.reject(new Error(raw)) }, onTrack);
    const card = await screen.findByRole("alertdialog", { name: "Could not join the cast" });
    expect(card.textContent).toContain("Try again in a moment.");
    expect(card.textContent).not.toContain("502");
    expect(again()).toHaveLength(1);
    expect(onTrack).toHaveBeenCalledWith("cast_watch_refused", { kind: "error", reason: raw }, expect.anything());
    expect(said).toHaveBeenCalledWith("Cast join failed:", raw);
  });

  it("says it reconnects, without LIVE, while the Cast does not arrive", async () => {
    const container = await joined({ signal: "reconnecting" });
    expect((await screen.findByRole("status")).textContent).toContain("Reconnecting");
    expect(screen.queryByText("LIVE")).toBeNull();
    expect(container.querySelector("video")).not.toBeNull();
    expect(container.querySelector(".cast-watch")?.getAttribute("data-state")).toBe("reconnecting");
  });

  it("gives no Dismiss button to a notice that stays while its cause stays", async () => {
    await joined({ signal: "reconnecting", audioBlocked: true });
    expect(await screen.findAllByRole("status")).toHaveLength(2);
    expect(screen.queryByRole("button", { name: "Dismiss" })).toBeNull();
    expect(screen.getByRole("button", { name: "Enable audio" })).toBeTruthy();
  });

  it("closes the notice of a video the browser cannot play on Dismiss, and shows it again after a new join", async () => {
    await joined({ signal: "undecodable" });
    fireEvent.click(await screen.findByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.getByRole("button", { name: "Leave" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Leave" }));
    fireEvent.click(screen.getByRole("button", { name: "JOIN NOW" }));
    expect((await screen.findByRole("status")).textContent).toContain("This browser cannot play the video");
  });

  it("shows the stage with no LIVE and no notice while the picture of the Cast has not arrived", async () => {
    const container = await joined({ signal: "arriving" });
    await screen.findByRole("button", { name: "Leave" });
    expect(screen.queryByText("LIVE")).toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
    expect(container.querySelector("video")).not.toBeNull();
  });

  it("says in words, without LIVE, that this browser cannot play the video of the Cast", async () => {
    await joined({ signal: "undecodable" });
    const notice = (await screen.findByRole("status")).textContent;
    expect(notice).toContain("This browser cannot play the video of this Cast yet");
    expect(notice).toContain("another browser");
    expect(screen.queryByText("LIVE")).toBeNull();
    expect(screen.getByRole("button", { name: "Leave" })).toBeTruthy();
  });

  it("joins again by itself after a lost connection, three times with longer waits, then says so on the end card", async () => {
    const join = vi.fn<WatchLive["join"]>(async () => Promise.reject(new Error("offline"))).mockResolvedValueOnce({ roomId: "room-1" });
    const lose = await watching({ join });
    lose("network");
    for (const [n, ms] of [[1, 2_000], [2, 5_000], [3, 10_000]] as const) {
      expect(tries(n)).toBeTruthy();
      expect(screen.queryByText("LIVE")).toBeNull();
      await pass(ms - 1);
      expect(join).toHaveBeenCalledTimes(n);
      await pass(1);
      expect(join).toHaveBeenCalledTimes(n + 1);
      expect(join).toHaveBeenLastCalledWith(true);
    }
    const card = screen.getByRole("alertdialog", { name: "The connection was lost" });
    expect(document.activeElement).toBe(card);
    await pass(60_000);
    expect(join).toHaveBeenCalledTimes(4);
    fireEvent.click(again()[0]!);
    expect(join).toHaveBeenCalledTimes(5);
    expect(join).toHaveBeenLastCalledWith(false);
  });

  it("leaves from the reconnecting view to the lobby, and no join follows", async () => {
    const join = vi.fn<WatchLive["join"]>(async () => ({ roomId: "room-1" }));
    const leave = vi.fn();
    const lose = await watching({ join, leave });
    lose("network");
    fireEvent.click(screen.getByRole("button", { name: "Leave" }));
    expect(screen.getByRole("button", { name: "JOIN NOW" })).toBeTruthy();
    expect(leave).toHaveBeenCalledTimes(1);
    await pass(60_000);
    expect(join).toHaveBeenCalledTimes(1);
  });

  it.each([
    { signal: "live", second: 1 },
    { signal: "none", second: 2 },
  ] as const)("counts the next loss as try $second after a try that joined with the Cast $signal", async ({ signal, second }) => {
    const lose = await watching({ signal });
    lose("network");
    expect(tries(1)).toBeTruthy();
    await pass(2_000);
    expect(screen.getByRole("button", { name: "Leave" })).toBeTruthy();
    lose("network");
    expect(tries(second)).toBeTruthy();
  });

  it("shows try 1 of a room that is lost in the commit that shows the Cast, before the page's effects run", async () => {
    let lost = (_why: WatchLoss) => {};
    let drop = true;
    const stage = () => <video ref={(el) => void (el && drop && ((drop = false), lost("network")))} />;
    await joined({ stage, bindLost: (onLost) => ((lost = onLost), () => {}) });
    expect(await screen.findByText("Reconnecting\u{2026} (1 of 3)")).toBeTruthy();
    expect(screen.getByText("Reconnecting\u{2026} (1 of 3)").closest(".cast-watch")?.getAttribute("data-state")).toBe("reconnecting");
  });

  it("shows the end card and does not join again by itself when the server removed the watcher", async () => {
    const join = vi.fn<WatchLive["join"]>(async () => ({ roomId: "room-1" }));
    const lose = await watching({ join });
    lose("removed");
    const card = screen.getByRole("alertdialog", { name: "You were removed from this Cast" });
    expect(document.activeElement).toBe(card);
    expect(screen.queryByText("LIVE")).toBeNull();
    expect(again()).toHaveLength(1);
    await pass(60_000);
    expect(join).toHaveBeenCalledTimes(1);
  });

  it("ends the tries with the end card, and no Join again for a ban, when a new join is refused", async () => {
    const refused = { kind: "error", title: "You cannot join this Cast", message: "You are banned from this scene", again: false } as const;
    const join = vi.fn<WatchLive["join"]>(async () => ({ refused })).mockResolvedValueOnce({ roomId: "room-1" });
    const lose = await watching({ join });
    lose("network");
    await pass(2_000);
    expect(screen.getByRole("alertdialog", { name: refused.title }).textContent).toContain(refused.message);
    expect(again()).toHaveLength(0);
    await pass(60_000);
    expect(join).toHaveBeenCalledTimes(2);
  });
});
