// The wearable-preview iframe on its own: what reaches the preview as an update, what needs a new mount, and how long a
// mount may stay silent.

import { act, cleanup, render, screen } from "@testing-library/react";
import { useEffect } from "react";
import { afterEach, describe, expect, test, vi } from "vitest";

import PreviewFrame, { type PreviewFrameOptions } from "./PreviewFrame";

const MAINNET = "https://wearable-preview.decentraland.org";
const TESTNET = "https://wearable-preview.decentraland.zone";
const HOODIE = "urn:decentraland:matic:collections-v2:0x0768847951fddd4e73e670800da26d953a77059b:0";
const HAT = "urn:decentraland:matic:collections-v2:0x0768847951fddd4e73e670800da26d953a77059b:1";
const DANCE = "urn:decentraland:matic:collections-v2:0x0768847951fddd4e73e670800da26d953a77059b:2";
const TESTNET_HAT = "urn:decentraland:amoy:collections-v2:0x0768847951fddd4e73e670800da26d953a77059b:1";

const UNITY: Partial<PreviewFrameOptions> = { unity: true, unityMode: "marketplace" };

const frame = () => screen.getByTitle("3D preview") as HTMLIFrameElement;
const params = () => new URL(frame().src).searchParams;

function fromPreview(data: unknown, origin = MAINNET) {
  act(() => {
    window.dispatchEvent(new MessageEvent("message", { data, source: frame().contentWindow, origin }));
  });
}

function mount(options: PreviewFrameOptions, loadTimeoutMs?: number) {
  const onLoad = vi.fn();
  const onError = vi.fn();
  const view = render(<PreviewFrame options={options} loadTimeoutMs={loadTimeoutMs} onLoad={onLoad} onError={onError} />);
  const change = (next: PreviewFrameOptions) =>
    view.rerender(<PreviewFrame options={next} loadTimeoutMs={loadTimeoutMs} onLoad={onLoad} onError={onError} />);
  return { onLoad, onError, change };
}

function booted() {
  const win = frame().contentWindow!;
  const posted = vi.spyOn(win, "postMessage");
  fromPreview({ type: "ready" });
  const updates = () => posted.mock.calls.filter(([message]) => (message as { type?: string }).type === "update").map(([message]) => message as { payload: { options: Record<string, unknown> } });
  return { win, updates };
}

/** What wearable-preview runs on: the mount URL's options with the update laid over them (its useOptions). */
function applied(update: Record<string, unknown>): Record<string, unknown> {
  return { ...Object.fromEntries(params()), ...update };
}

/** document.hidden under the test's control, with the event a real tab sends when it changes. */
function hiddenPage(hidden: boolean) {
  let state = hidden;
  Object.defineProperty(document, "hidden", { configurable: true, get: () => state });
  const set = (next: boolean) => {
    state = next;
    act(() => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
  };
  // Without act: for a change queued on the fake clock, which runs inside the test's own act.
  const now = (next: boolean) => {
    state = next;
    document.dispatchEvent(new Event("visibilitychange"));
  };
  return { show: () => set(false), hide: () => set(true), now, restore: () => delete (document as { hidden?: boolean }).hidden };
}

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("updates", () => {
  test("an update never carries the renderer's options, which Unity reads from the mount URL only", () => {
    const { change } = mount({ urns: [HOODIE], profile: "default", ...UNITY });
    expect(params().get("unity")).toBe("true");
    expect(params().get("mode")).toBe("marketplace");
    const { win, updates } = booted();

    change({ urns: [HAT], profile: "default", ...UNITY });
    expect(frame().contentWindow).toBe(win);
    expect(updates()).toHaveLength(1);
    expect(updates()[0]!.payload.options).toMatchObject({ urns: [HAT], profile: "default" });
    expect("unity" in updates()[0]!.payload.options).toBe(false);
    expect("unityMode" in updates()[0]!.payload.options).toBe(false);
  });

  test("over Babylon an option left unset goes as an update, which Babylon takes whole", () => {
    const { change } = mount({ urns: [HOODIE], profile: "default", emote: "fashion", type: "avatar" });
    const { win, updates } = booted();

    change({ urns: [HOODIE], profile: undefined, emote: undefined, type: "wearable" });
    expect(frame().contentWindow).toBe(win);
    expect(updates()).toHaveLength(1);
    expect(updates()[0]!.payload.options).toMatchObject({ type: "wearable" });
    expect(Object.hasOwn(updates()[0]!.payload.options, "emote")).toBe(true);
    expect(applied(updates()[0]!.payload.options).emote).toBeUndefined();
  });

  test("over Babylon an option the mount URL set and the new options leave out is cleared, not taken back from the URL", () => {
    const { change } = mount({ urns: [HOODIE], profile: "default", emote: "fashion", type: "avatar" });
    const { win, updates } = booted();

    change({ urns: [HAT] });
    expect(frame().contentWindow).toBe(win);
    const first = updates()[0]!.payload.options;
    for (const key of ["profile", "emote", "type"]) {
      expect(Object.hasOwn(first, key)).toBe(true);
      expect(applied(first)[key]).toBeUndefined();
    }

    // Every later update clears them again: each one replaces the last whole.
    change({ urns: [HOODIE] });
    const second = updates()[1]!.payload.options;
    expect(second.urns).toEqual([HOODIE]);
    expect(Object.hasOwn(second, "profile")).toBe(true);
    expect(applied(second).profile).toBeUndefined();
  });
});

describe("a new mount", () => {
  test("under Unity, leaving unset an option the mount URL set mounts again with the new options", () => {
    const { change } = mount({ urns: [DANCE], profile: "default", wheelZoom: 1.5, wheelStart: 100, ...UNITY });
    const { win, updates } = booted();

    // Unity skips an unset key in an update, so the closer zoom would stay on the next item.
    change({ urns: [HAT], profile: "default", emote: "fashion", ...UNITY });
    expect(updates()).toHaveLength(0);
    expect(frame().contentWindow).not.toBe(win);
    expect(params().getAll("urn")).toEqual([HAT]);
    expect(params().has("wheelZoom")).toBe(false);
    expect(params().get("emote")).toBe("fashion");
    expect(params().get("unity")).toBe("true");
  });

  test("under Unity, leaving unset an option an earlier update set mounts again too", () => {
    const { change } = mount({ urns: [HOODIE], profile: "default", ...UNITY });
    const first = booted();

    change({ urns: [HOODIE], profile: "default", emote: "fashion", ...UNITY });
    expect(first.updates()).toHaveLength(1);
    expect(frame().contentWindow).toBe(first.win);

    change({ urns: [HAT], profile: "default", ...UNITY });
    expect(first.updates()).toHaveLength(1);
    expect(frame().contentWindow).not.toBe(first.win);
    expect(params().has("emote")).toBe(false);

    // The new mount holds only its URL's options: a later change it can carry goes as an update again.
    const second = booted();
    change({ urns: [HOODIE], profile: "default", ...UNITY });
    expect(second.updates()).toHaveLength(1);
    expect(frame().contentWindow).toBe(second.win);
  });

  test("an item on another network mounts the preview that reads it, and only its messages count", () => {
    const { change, onLoad } = mount({ urns: [HOODIE], profile: "default" });
    booted();

    change({ urns: [TESTNET_HAT], profile: "default" });
    expect(frame().src.startsWith(`${TESTNET}/?`)).toBe(true);
    expect(params().get("env")).toBe("dev");
    fromPreview({ type: "load", payload: { renderer: "babylon" } }, MAINNET);
    expect(onLoad).not.toHaveBeenCalled();
    fromPreview({ type: "load", payload: { renderer: "babylon" } }, TESTNET);
    expect(onLoad).toHaveBeenCalledWith("babylon");
  });

  test("a retiring iframe cannot answer for the one replacing it", () => {
    vi.useFakeTimers();
    const onLoad = vi.fn();
    const onError = vi.fn();
    const retiring: { win: Window | null } = { win: null };
    // Its effect runs after PreviewFrame's in the same commit: the frame has chosen a new mount, and React has not
    // replaced the iframe yet.
    function Page({ options }: { options: PreviewFrameOptions }) {
      useEffect(() => {
        if (!retiring.win) return;
        window.dispatchEvent(new MessageEvent("message", { data: { type: "load", payload: { renderer: "unity" } }, source: retiring.win, origin: MAINNET }));
      }, [options]);
      return <PreviewFrame options={options} loadTimeoutMs={45_000} onLoad={onLoad} onError={onError} />;
    }

    const view = render(<Page options={{ urns: [DANCE], emote: "fashion", ...UNITY }} />);
    fromPreview({ type: "ready" });
    retiring.win = frame().contentWindow;
    view.rerender(<Page options={{ urns: [HAT], ...UNITY }} />);

    expect(frame().contentWindow).not.toBe(retiring.win);
    expect(onLoad).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(45_000));
    expect(onError).toHaveBeenCalledTimes(1);
  });

  test("a change of renderer mounts again", () => {
    const { change } = mount({ urns: [HOODIE], ...UNITY });
    const { win, updates } = booted();
    change({ urns: [HOODIE] });
    expect(updates()).toHaveLength(0);
    expect(frame().contentWindow).not.toBe(win);
    expect(params().has("unity")).toBe(false);
  });

  test("a change made before the preview boots waits for it, unless it needs a new mount", () => {
    const { change } = mount({ urns: [HOODIE], profile: "default", ...UNITY });
    const win = frame().contentWindow!;
    const posted = vi.spyOn(win, "postMessage");
    change({ urns: [HAT], profile: "default", ...UNITY });
    expect(posted).not.toHaveBeenCalled();
    fromPreview({ type: "ready" });
    expect(posted.mock.calls.at(-1)![0]).toMatchObject({ type: "update", payload: { options: { urns: [HAT] } } });

    change({ urns: [HAT], ...UNITY });
    expect(frame().contentWindow).not.toBe(win);
  });
});

describe("a mount that stays silent", () => {
  test("counts as failed once the load timeout passes without a load or an error", () => {
    vi.useFakeTimers();
    const { onError } = mount({ urns: [HOODIE], ...UNITY }, 45_000);
    fromPreview({ type: "ready" });
    act(() => vi.advanceTimersByTime(44_999));
    expect(onError).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(1));
    expect(onError).toHaveBeenCalledTimes(1);
  });

  test("does not, once it has drawn or said it failed", () => {
    vi.useFakeTimers();
    const drew = mount({ urns: [HOODIE], ...UNITY }, 45_000);
    fromPreview({ type: "load", payload: { renderer: "unity" } });
    act(() => vi.advanceTimersByTime(60_000));
    expect(drew.onError).not.toHaveBeenCalled();
    cleanup();

    const failed = mount({ urns: [HOODIE], ...UNITY }, 45_000);
    fromPreview({ type: "error", payload: { message: "no config" } });
    act(() => vi.advanceTimersByTime(60_000));
    expect(failed.onError).toHaveBeenCalledTimes(1);
  });

  test("a new mount gets the whole timeout again", () => {
    vi.useFakeTimers();
    const { change, onError } = mount({ urns: [DANCE], wheelZoom: 1.5, ...UNITY }, 45_000);
    act(() => vi.advanceTimersByTime(40_000));
    change({ urns: [HAT], ...UNITY });
    act(() => vi.advanceTimersByTime(40_000));
    expect(onError).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(5_000));
    expect(onError).toHaveBeenCalledTimes(1);
  });

  test("counts only the time the page is visible, where the preview can draw", () => {
    vi.useFakeTimers();
    const page = hiddenPage(true);
    try {
      const { onError } = mount({ urns: [HOODIE], ...UNITY }, 45_000);
      act(() => vi.advanceTimersByTime(10 * 60_000));
      expect(onError).not.toHaveBeenCalled();

      page.show();
      act(() => vi.advanceTimersByTime(30_000));
      page.hide();
      act(() => vi.advanceTimersByTime(10 * 60_000));
      page.show();
      act(() => vi.advanceTimersByTime(14_999));
      expect(onError).not.toHaveBeenCalled();
      act(() => vi.advanceTimersByTime(1));
      expect(onError).toHaveBeenCalledTimes(1);

      // Spent: hiding and showing the page again does not time it once more.
      page.hide();
      page.show();
      act(() => vi.advanceTimersByTime(60_000));
      expect(onError).toHaveBeenCalledTimes(1);
    } finally {
      page.restore();
    }
  });

  test("a page hidden just as the visible time runs out, before the timeout runs, has it run as soon as it is shown", () => {
    vi.useFakeTimers();
    const page = hiddenPage(false);
    try {
      // Queued before the mount, so at 45 s it runs ahead of the frame's own timeout.
      setTimeout(() => page.now(true), 45_000);
      const { onError } = mount({ urns: [HOODIE], ...UNITY }, 45_000);
      act(() => vi.advanceTimersByTime(45_000));
      expect(onError).not.toHaveBeenCalled();

      page.show();
      act(() => vi.advanceTimersByTime(0));
      expect(onError).toHaveBeenCalledTimes(1);
      act(() => vi.advanceTimersByTime(100_000));
      page.hide();
      page.show();
      act(() => vi.advanceTimersByTime(100_000));
      expect(onError).toHaveBeenCalledTimes(1);
    } finally {
      page.restore();
    }
  });

  test("without a timeout, waits as long as it takes", () => {
    vi.useFakeTimers();
    const { onError } = mount({ urns: [HOODIE] });
    act(() => vi.advanceTimersByTime(10 * 60_000));
    expect(onError).not.toHaveBeenCalled();
  });
});
