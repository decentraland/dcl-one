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

/** The preview boots and, given a renderer, draws its first scene: Unity takes no update before that. */
function booted(drew?: "unity" | "babylon") {
  const win = frame().contentWindow!;
  const posted = vi.spyOn(win, "postMessage");
  fromPreview({ type: "ready" });
  if (drew) fromPreview({ type: "load", payload: { renderer: drew } });
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
    const { win, updates } = booted("unity");

    change({ urns: [HAT], profile: "default", ...UNITY });
    expect(frame().contentWindow).toBe(win);
    expect(updates()).toHaveLength(1);
    expect(updates()[0]!.payload.options).toMatchObject({ urns: [HAT], profile: "default" });
    expect("unity" in updates()[0]!.payload.options).toBe(false);
    expect("unityMode" in updates()[0]!.payload.options).toBe(false);
  });

  test("under Unity an update carries only the options Unity applies from one; the rest stay on the mount URL", () => {
    const { change } = mount({ urns: [HOODIE], profile: "default", disableFadeEffect: true, ...UNITY });
    const { win, updates } = booted("unity");

    change({ urns: [HAT], profile: "default", disableFadeEffect: true, ...UNITY });
    expect(frame().contentWindow).toBe(win);
    expect(updates()).toHaveLength(1);
    expect(updates()[0]!.payload.options).toMatchObject({ urns: [HAT], profile: "default" });
    expect(Object.keys(updates()[0]!.payload.options)).not.toContain("disableFadeEffect");

    // A change to such an option cannot go as an update: it mounts again.
    change({ urns: [HAT], profile: "default", ...UNITY });
    expect(updates()).toHaveLength(1);
    expect(frame().contentWindow).not.toBe(win);
    expect(params().has("disableFadeEffect")).toBe(false);
  });

  test("under Unity the next update waits for the answer to the last, and an answer for a scene the page left is not passed on", () => {
    const { change, onLoad, onError } = mount({ urns: [HOODIE], profile: "default", ...UNITY });
    const { updates } = booted("unity");
    expect(onLoad).toHaveBeenCalledTimes(1);

    change({ urns: [HAT], profile: "default", ...UNITY });
    change({ urns: [HOODIE, HAT], profile: "default", ...UNITY });
    expect(updates()).toHaveLength(1);

    // The answer for the scene before: not the page's, and the next scene goes now.
    fromPreview({ type: "load", payload: { renderer: "unity" } });
    expect(onLoad).toHaveBeenCalledTimes(1);
    expect(updates()).toHaveLength(2);
    expect(updates()[1]!.payload.options.urns).toEqual([HOODIE, HAT]);

    fromPreview({ type: "load", payload: { renderer: "unity" } });
    expect(onLoad).toHaveBeenCalledTimes(2);

    // An error for a scene the page has left takes nothing down either.
    change({ urns: [HAT], profile: "default", ...UNITY });
    change({ urns: [HOODIE], profile: "default", ...UNITY });
    fromPreview({ type: "error", payload: { message: "not found" } });
    expect(onError).not.toHaveBeenCalled();
    expect(updates().at(-1)!.payload.options.urns).toEqual([HOODIE]);
  });

  test("under Unity a scene that never answers holds the next one only for a while, which then gets a fresh iframe", () => {
    vi.useFakeTimers();
    const { change, onLoad, onError } = mount({ urns: [HOODIE], profile: "default", ...UNITY });
    const { win, updates } = booted("unity");
    onLoad.mockClear();
    change({ urns: [HAT], profile: "default", ...UNITY });
    change({ urns: [HOODIE, HAT], profile: "default", ...UNITY });
    expect(updates()).toHaveLength(1);
    act(() => vi.advanceTimersByTime(2_999));
    expect(updates()).toHaveLength(1);
    expect(frame().contentWindow).toBe(win);

    act(() => vi.advanceTimersByTime(1));
    expect(updates()).toHaveLength(1);
    expect(frame().contentWindow).not.toBe(win);
    expect(params().getAll("urn")).toEqual([HOODIE, HAT]);

    // The retired iframe's late answers, for the scene before, count for nothing.
    for (const data of [{ type: "load", payload: { renderer: "unity" } }, { type: "error", payload: { message: "late" } }]) {
      act(() => {
        window.dispatchEvent(new MessageEvent("message", { data, source: win, origin: MAINNET }));
      });
    }
    expect(onLoad).not.toHaveBeenCalled();
    expect(onError).not.toHaveBeenCalled();
    fromPreview({ type: "ready" });
    fromPreview({ type: "load", payload: { renderer: "unity" } });
    expect(onLoad).toHaveBeenCalledTimes(1);
  });

  test("under Unity a frame that boots again is replaced by a fresh one, whose start failing is reported", () => {
    const { change, onLoad, onError } = mount({ urns: [HOODIE], profile: "default", ...UNITY }, 45_000);
    const { win, updates } = booted("unity");
    change({ urns: [HAT], profile: "default", ...UNITY });
    expect(updates()).toHaveLength(1);
    fromPreview({ type: "load", payload: { renderer: "unity" } });
    expect(onLoad).toHaveBeenCalledTimes(2);

    // The same iframe boots again: it has lost its scene, and the next one is drawn in a fresh iframe.
    fromPreview({ type: "ready" });
    expect(frame().contentWindow).not.toBe(win);
    expect(params().getAll("urn")).toEqual([HAT]);

    change({ urns: [HOODIE, HAT], profile: "default", ...UNITY });
    fromPreview({ type: "ready" });
    fromPreview({ type: "error", payload: { message: "unity could not start" } });
    expect(onError).toHaveBeenCalledTimes(1);
  });

  test("under Unity the fresh frame after a second boot has a load deadline of its own", () => {
    vi.useFakeTimers();
    const { onError } = mount({ urns: [HOODIE], profile: "default", ...UNITY }, 45_000);
    booted("unity");
    act(() => vi.advanceTimersByTime(30_000));
    fromPreview({ type: "ready" });
    act(() => vi.advanceTimersByTime(44_999));
    expect(onError).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(1));
    expect(onError).toHaveBeenCalledTimes(1);
  });

  test("over Babylon an update goes at once: Babylon drops the answer for a scene it was sent past", () => {
    const { change } = mount({ urns: [HOODIE], profile: "default" });
    const { updates } = booted("babylon");
    change({ urns: [HAT], profile: "default" });
    change({ urns: [HOODIE, HAT], profile: "default" });
    expect(updates()).toHaveLength(2);
  });

  test("over Babylon the same option still goes in every update", () => {
    const { change } = mount({ urns: [HOODIE], profile: "default", disableFadeEffect: true });
    const { updates } = booted();
    change({ urns: [HAT], profile: "default", disableFadeEffect: true });
    expect(updates()[0]!.payload.options).toMatchObject({ urns: [HAT], disableFadeEffect: true });
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
    const first = booted("unity");

    change({ urns: [HOODIE], profile: "default", emote: "fashion", ...UNITY });
    expect(first.updates()).toHaveLength(1);
    expect(frame().contentWindow).toBe(first.win);

    change({ urns: [HAT], profile: "default", ...UNITY });
    expect(first.updates()).toHaveLength(1);
    expect(frame().contentWindow).not.toBe(first.win);
    expect(params().has("emote")).toBe(false);

    // The new mount holds only its URL's options: a later change it can carry goes as an update again.
    const second = booted("unity");
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
    const { change } = mount({ urns: [HOODIE], profile: "default" });
    const win = frame().contentWindow!;
    const posted = vi.spyOn(win, "postMessage");
    change({ urns: [HAT], profile: "default" });
    expect(posted).not.toHaveBeenCalled();
    fromPreview({ type: "ready" });
    expect(posted.mock.calls.at(-1)![0]).toMatchObject({ type: "update", payload: { options: { urns: [HAT] } } });

    change({ urns: [HAT], ...UNITY });
    expect(frame().contentWindow).not.toBe(win);
  });

  test("under Unity a change made before the first scene waits for that scene's answer, which is not the change's", () => {
    const { change, onLoad } = mount({ urns: [HOODIE], profile: "default", ...UNITY });
    const posted = vi.spyOn(frame().contentWindow!, "postMessage");
    change({ urns: [HAT], profile: "default", ...UNITY });
    fromPreview({ type: "ready" });
    // Unity applies no update before its first scene has loaded.
    expect(posted).not.toHaveBeenCalled();

    fromPreview({ type: "load", payload: { renderer: "unity" } });
    expect(onLoad).not.toHaveBeenCalled();
    expect(posted.mock.calls.at(-1)![0]).toMatchObject({ type: "update", payload: { options: { urns: [HAT] } } });
    fromPreview({ type: "load", payload: { renderer: "unity" } });
    expect(onLoad).toHaveBeenCalledTimes(1);
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

describe("ambient playback", () => {
  const controls = (posted: { mock: { calls: unknown[][] } }) =>
    posted.mock.calls
      .map(([message]) => message as { type?: string; payload?: { namespace?: string; method?: string } })
      .filter((message) => message.type === "controller_request")
      .map((message) => `${message.payload?.namespace}.${message.payload?.method}`);

  test("mutes each scene it draws and plays its emote again each time it ends", () => {
    render(<PreviewFrame options={{ urns: [HOODIE], emote: "fashion" }} ambient onLoad={vi.fn()} onError={vi.fn()} />);
    const posted = vi.spyOn(frame().contentWindow!, "postMessage");
    fromPreview({ type: "ready" });
    expect(controls(posted)).toEqual([]);
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(controls(posted)).toEqual(["emote.disableSound"]);
    fromPreview({ type: "emote_event", payload: { type: "animation_loop" } });
    expect(controls(posted)).toEqual(["emote.disableSound"]);
    fromPreview({ type: "emote_event", payload: { type: "animation_end" } });
    fromPreview({ type: "emote_event", payload: { type: "animation_end" } });
    expect(controls(posted)).toEqual(["emote.disableSound", "emote.play", "emote.play"]);
  });

  test("an emote ending in another preview on the page plays nothing here", () => {
    render(<PreviewFrame options={{ urns: [HOODIE] }} ambient onLoad={vi.fn()} onError={vi.fn()} />);
    const posted = vi.spyOn(frame().contentWindow!, "postMessage");
    fromPreview({ type: "ready" });
    act(() => {
      window.dispatchEvent(new MessageEvent("message", { data: { type: "emote_event", payload: { type: "animation_end" } }, source: window, origin: MAINNET }));
    });
    expect(controls(posted)).toEqual([]);
  });

  test("off by default: a scene keeps its sound and its emote plays once", () => {
    const { onLoad } = mount({ urns: [HOODIE] });
    const posted = vi.spyOn(frame().contentWindow!, "postMessage");
    fromPreview({ type: "ready" });
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    fromPreview({ type: "emote_event", payload: { type: "animation_end" } });
    expect(onLoad).toHaveBeenCalledWith("babylon");
    expect(controls(posted)).toEqual([]);
  });
});

describe("the Bevy avatar preview", () => {
  const BEVY = () => window.location.origin;
  const fromBevy = (data: unknown) => fromPreview(data, BEVY());

  test("mounts this site's /avatar-preview/ with the same options, Unity's mode among them, and not the renderer switch", () => {
    mount({ urns: [HOODIE], profile: "default", unityMode: "builder", bevy: true });
    const src = new URL(frame().src);
    expect(`${src.origin}${src.pathname}`).toBe(`${BEVY()}/avatar-preview/`);
    expect(params().getAll("urn")).toEqual([HOODIE]);
    expect(params().get("mode")).toBe("builder");
    expect(params().has("bevy")).toBe(false);
    expect(params().has("unity")).toBe(false);
    expect(params().has("env")).toBe(false);
  });

  test("testnet items go to the same page told to read testnet, and moving between networks mounts again", () => {
    const { change } = mount({ urns: [TESTNET_HAT], bevy: true });
    expect(new URL(frame().src).origin).toBe(BEVY());
    expect(params().get("env")).toBe("dev");
    const first = frame().contentWindow;
    fromBevy({ type: "ready" });
    fromBevy({ type: "load", payload: { renderer: "bevy" } });
    change({ urns: [HAT], bevy: true });
    expect(frame().contentWindow).not.toBe(first);
    expect(params().has("env")).toBe(false);
  });

  test("its answers count from this site's origin, and name Bevy", () => {
    const { onLoad } = mount({ urns: [HOODIE], bevy: true });
    fromPreview({ type: "load", payload: { renderer: "bevy" } }, MAINNET);
    expect(onLoad).not.toHaveBeenCalled();
    fromBevy({ type: "load", payload: { renderer: "bevy" } });
    expect(onLoad).toHaveBeenCalledWith("bevy");
  });

  test("an update goes whole, and scenes go one at a time: the answer for one the page left is not passed on", () => {
    const { onLoad, change } = mount({ urns: [HOODIE], disableFadeEffect: true, bevy: true });
    const win = frame().contentWindow!;
    const posted = vi.spyOn(win, "postMessage");
    const updates = () => posted.mock.calls.filter(([message]) => (message as { type?: string }).type === "update").map(([message]) => (message as { payload: { options: Record<string, unknown> } }).payload.options);
    fromBevy({ type: "ready" });
    change({ urns: [HAT], disableFadeEffect: true, bevy: true });
    // Before the first scene answers, nothing is sent: its answer could not be told from the change's.
    expect(updates()).toEqual([]);
    fromBevy({ type: "load", payload: { renderer: "bevy" } });
    expect(onLoad).not.toHaveBeenCalled();
    expect(updates()).toHaveLength(1);
    expect(updates()[0]).toMatchObject({ urns: [HAT], disableFadeEffect: true });
    expect(Object.keys(updates()[0]!)).not.toContain("bevy");

    change({ urns: [HOODIE, HAT], disableFadeEffect: true, bevy: true });
    expect(updates()).toHaveLength(1);
    fromBevy({ type: "load", payload: { renderer: "bevy" } });
    expect(onLoad).not.toHaveBeenCalled();
    expect(updates()).toHaveLength(2);
    fromBevy({ type: "load", payload: { renderer: "bevy" } });
    expect(onLoad).toHaveBeenCalledTimes(1);
    expect(frame().contentWindow).toBe(win);
  });

  test("a start that fails is reported whatever the page changed meanwhile, and nothing more is sent to it", () => {
    const { onError, change } = mount({ urns: [HOODIE], bevy: true });
    const posted = vi.spyOn(frame().contentWindow!, "postMessage");
    fromBevy({ type: "ready" });
    change({ urns: [HAT], bevy: true });
    fromBevy({ type: "error", payload: { message: "renderer failed to start" } });
    expect(onError).toHaveBeenCalledWith("renderer failed to start");
    expect(posted.mock.calls.filter(([message]) => (message as { type?: string }).type === "update")).toEqual([]);
  });

  test("an error answering a scene the page has left may be the renderer dying: the latest options go to a fresh iframe with its own deadline", () => {
    vi.useFakeTimers();
    const { onError, change } = mount({ urns: [HOODIE], bevy: true }, 45_000);
    const first = frame().contentWindow;
    fromBevy({ type: "ready" });
    fromBevy({ type: "load", payload: { renderer: "bevy" } });
    change({ urns: [HAT], bevy: true });
    change({ urns: [HOODIE, HAT], bevy: true });
    fromBevy({ type: "error", payload: { message: "the renderer panicked" } });
    expect(frame().contentWindow).not.toBe(first);
    expect(params().getAll("urn")).toEqual([HOODIE, HAT]);
    expect(onError).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(45_000));
    expect(onError).toHaveBeenCalledWith("The preview did not load in time.");
  });

  test("an option an earlier update set and the next one leaves out is cleared, since Bevy keeps every update's", () => {
    const { change } = mount({ urns: [HOODIE], bevy: true });
    const posted = vi.spyOn(frame().contentWindow!, "postMessage");
    const updates = () => posted.mock.calls.filter(([message]) => (message as { type?: string }).type === "update").map(([message]) => (message as { payload: { options: Record<string, unknown> } }).payload.options);
    fromBevy({ type: "ready" });
    fromBevy({ type: "load", payload: { renderer: "bevy" } });
    change({ urns: [HOODIE], skin: "ff0000", bevy: true });
    fromBevy({ type: "load", payload: { renderer: "bevy" } });
    change({ urns: [HOODIE], bevy: true });
    // What Bevy runs on: the mount URL's options with every update laid over them in turn (preview.js state.overrides).
    const merged = updates().reduce<Record<string, unknown>>((all, update) => ({ ...all, ...update }), Object.fromEntries(params()));
    expect(updates()).toHaveLength(2);
    expect(Object.keys(updates()[1]!)).toContain("skin");
    expect(merged.skin).toBeUndefined();
  });

  test("turning Bevy off mounts wearable-preview", () => {
    const { change } = mount({ urns: [HOODIE], bevy: true });
    fromBevy({ type: "ready" });
    fromBevy({ type: "load", payload: { renderer: "bevy" } });
    change({ urns: [HOODIE] });
    expect(new URL(frame().src).origin).toBe(MAINNET);
  });
});
