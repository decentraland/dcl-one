// @vitest-environment jsdom
import "../locale/test-setup";
import { StrictMode } from "react";
import {
  act,
  render,
  renderHook,
  screen,
  waitFor,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const runDownload = vi.fn();
const track = vi.fn();

vi.mock("./success", async (actual) => ({
  ...(await actual<typeof import("./success")>()),
  runDownload: (...args: unknown[]) => runDownload(...args),
}));
vi.mock("@/core/lib/telemetry/track", () => ({
  track: (...args: unknown[]) => track(...args),
}));
vi.mock("@/data/lib/auth/session", () => ({ getIdentity: () => null }));

import { useDownloadSuccess } from "./useDownloadSuccess";

const SEARCH =
  "?os=Windows&arch=amd64&place=download-page&position=10%2C20&referrer=0x1111111111111111111111111111111111111111&utm_source=Partner";
const session = { sid: "sid-1", story: "download" };
const events = () => track.mock.calls.map((call) => call[0]);

beforeEach(() => {
  runDownload.mockReset().mockResolvedValue({ status: "success" });
  track.mockReset();
  sessionStorage.clear();
  localStorage.clear();
});

describe("useDownloadSuccess", () => {
  // render() with <StrictMode> in the tree: a renderHook wrapper does not double-run the effects.
  it("starts exactly one download and one arrival event under StrictMode", async () => {
    function Page() {
      return (
        <p>
          {
            useDownloadSuccess(SEARCH, session, "https://gateway.example").state
              .phase
          }
        </p>
      );
    }
    render(
      <StrictMode>
        <Page />
      </StrictMode>,
    );

    await screen.findByText("saved");
    expect(runDownload).toHaveBeenCalledTimes(1);
    expect(events()).toEqual(["download_success_arrived"]);
    const input = runDownload.mock.calls[0]![0];
    expect(input).toMatchObject({
      os: "windows",
      arch: "amd64",
      gateway: "https://gateway.example",
      identity: null,
      query: {
        position: "10,20",
        realm: undefined,
        referrer: "0x1111111111111111111111111111111111111111",
      },
    });
    expect(input.signal.aborted).toBe(false);
  });

  it("downloads once per navigation: a remount (back/forward) downloads again and counts a revisit", async () => {
    const first = renderHook(() =>
      useDownloadSuccess(SEARCH, session, undefined),
    );
    await waitFor(() => expect(first.result.current.state.phase).toBe("saved"));
    first.unmount();
    renderHook(() => useDownloadSuccess(SEARCH, session, undefined));

    await waitFor(() => expect(runDownload).toHaveBeenCalledTimes(2));
    const arrivals = track.mock.calls.filter(
      (call) => call[0] === "download_success_arrived",
    );
    expect(arrivals.map((call) => call[1].revisit)).toEqual([0, 1]);
  });

  it("aborts the running download when the page really unmounts", async () => {
    runDownload.mockReturnValue(new Promise(() => undefined));
    const { unmount } = renderHook(() =>
      useDownloadSuccess(SEARCH, session, undefined),
    );
    const signal: AbortSignal = runDownload.mock.calls[0]![0].signal;

    unmount();
    await waitFor(() => expect(signal.aborted).toBe(true));
  });

  it("re-downloads from the footer link with its own place, and ignores clicks while one runs", async () => {
    const { result } = renderHook(() =>
      useDownloadSuccess(SEARCH, session, undefined),
    );
    await waitFor(() => expect(result.current.state.phase).toBe("saved"));

    let release: (value: unknown) => void = () => undefined;
    runDownload.mockReturnValueOnce(
      new Promise((resolve) => (release = resolve)),
    );
    act(() => result.current.redownload());
    act(() => result.current.redownload());

    expect(runDownload).toHaveBeenCalledTimes(2);
    expect(track.mock.calls.filter((call) => call[0] === "Click")).toHaveLength(
      1,
    );
    expect(
      track.mock.calls.find((call) => call[0] === "Click")![1],
    ).toMatchObject({ place: "download-success-footer" });
    await act(async () => release({ status: "failed", reason: "x" }));
    expect(result.current.state).toEqual({ phase: "failed", reason: "x" });
  });

  it("sends download_funnel_exit on hide only for a known place", async () => {
    const hide = () => {
      Object.defineProperty(document, "visibilityState", {
        value: "hidden",
        configurable: true,
      });
      document.dispatchEvent(new Event("visibilitychange"));
    };
    const known = renderHook(() =>
      useDownloadSuccess(SEARCH, session, undefined),
    );
    await waitFor(() => expect(known.result.current.state.phase).toBe("saved"));
    hide();
    expect(events()).toContain("download_funnel_exit");
    known.unmount();

    track.mockReset();
    const unknown = renderHook(() =>
      useDownloadSuccess("?os=macos", session, undefined),
    );
    await waitFor(() =>
      expect(unknown.result.current.state.phase).toBe("saved"),
    );
    hide();
    expect(events()).not.toContain("download_funnel_exit");
  });
});
