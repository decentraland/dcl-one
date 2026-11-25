// The stage the fitting room and the outfit pages draw their avatar on: which renderer it asks for, what covers it, and
// where it goes when a renderer fails.

import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { primeRemoteFlags } from "../../discovery/lib/remoteFlags";
import AvatarStage from "./AvatarStage";
import type { PreviewFrameOptions } from "./PreviewFrame";

const PREVIEW = "https://wearable-preview.decentraland.org";
const HOODIE = "urn:decentraland:matic:collections-v2:0x0768847951fddd4e73e670800da26d953a77059b:0";
const HAT = "urn:decentraland:matic:collections-v2:0x0768847951fddd4e73e670800da26d953a77059b:1";
const MALE = "urn:decentraland:off-chain:base-avatars:BaseMale";

const env = vi.hoisted(() => ({ device: "full" as "full" | "lite", reason: "connection-ok" }));
vi.mock("../lib/pickRenderer", () => ({ pickRenderer: () => ({ renderer: env.device, reason: env.reason }) }));

type StageOptions = Omit<PreviewFrameOptions, "unity" | "unityMode">;
const OUTFIT: StageOptions = { urns: [HOODIE], profile: "default", bodyShape: MALE, type: "avatar", emote: "fashion", disableBackground: true };

const frame = () => screen.queryByTitle("3D preview") as HTMLIFrameElement | null;
const params = () => new URL(frame()!.src).searchParams;
const cover = () => screen.queryByTestId("cover");

function fromPreview(data: unknown) {
  act(() => {
    window.dispatchEvent(new MessageEvent("message", { data, source: frame()!.contentWindow, origin: PREVIEW }));
  });
}

function stage(options: StageOptions = OUTFIT, extra: { unavailable?: React.ReactNode } = {}) {
  const element = (next: StageOptions) => <AvatarStage options={next} cover={<span data-testid="cover" />} {...extra} />;
  const view = render(element(options));
  return { change: (next: StageOptions) => view.rerender(element(next)) };
}

beforeEach(() => {
  env.device = "full";
  env.reason = "connection-ok";
  primeRemoteFlags({ "dapps-unity-wearable-preview": true });
});
afterEach(() => {
  cleanup();
  primeRemoteFlags(null);
  vi.useRealTimers();
});

describe("the renderer", () => {
  test("Unity in builder mode where the flag and the device allow it, covered until it draws", () => {
    stage();
    expect(params().get("unity")).toBe("true");
    expect(params().get("mode")).toBe("builder");
    expect(params().getAll("urn")).toEqual([HOODIE]);
    expect(params().get("profile")).toBe("default");
    expect(cover()).not.toBeNull();
    fromPreview({ type: "load", payload: { renderer: "unity" } });
    expect(cover()).toBeNull();
  });

  test("Babylon with the flag off, and on a lighter device without waiting for the flag", () => {
    primeRemoteFlags({ "dapps-unity-wearable-preview": false });
    stage();
    expect(params().has("unity")).toBe(false);
    cleanup();

    primeRemoteFlags(null);
    env.device = "lite";
    env.reason = "mobile";
    stage();
    expect(frame()).not.toBeNull();
    expect(params().has("unity")).toBe(false);
  });

  test("without WebGL nothing mounts and the unavailable state shows", () => {
    env.device = "lite";
    env.reason = "no-webgl";
    stage(OUTFIT, { unavailable: <span data-testid="unavailable" /> });
    expect(frame()).toBeNull();
    expect(screen.queryByTestId("unavailable")).not.toBeNull();
  });
});

describe("failures", () => {
  test("Unity failing gives way to Babylon, which stays for later changes", () => {
    const { change } = stage();
    fromPreview({ type: "error", payload: { message: "unity failed" } });
    expect(params().has("unity")).toBe(false);
    fromPreview({ type: "ready" });
    change({ ...OUTFIT, urns: [HAT] });
    expect(params().has("unity")).toBe(false);
  });

  test("Unity silent for 45 seconds gives way to Babylon, which has no timeout", () => {
    vi.useFakeTimers();
    stage();
    act(() => vi.advanceTimersByTime(45_000));
    expect(params().has("unity")).toBe(false);
    act(() => vi.advanceTimersByTime(10 * 60_000));
    expect(frame()).not.toBeNull();
    expect(cover()).not.toBeNull();
  });

  test("Babylon failing takes the stage down for those items, and the next change tries again", () => {
    primeRemoteFlags({ "dapps-unity-wearable-preview": false });
    const { change } = stage(OUTFIT, { unavailable: <span data-testid="unavailable" /> });
    fromPreview({ type: "error", payload: { message: "not found" } });
    expect(frame()).toBeNull();
    expect(screen.queryByTestId("unavailable")).not.toBeNull();

    change({ ...OUTFIT, urns: [HAT] });
    expect(frame()).not.toBeNull();
    expect(params().getAll("urn")).toEqual([HAT]);
    expect(cover()).not.toBeNull();
  });
});

describe("coming back to options seen before", () => {
  test("drawn, then away to options that failed: back again it is covered until the new iframe draws", () => {
    primeRemoteFlags({ "dapps-unity-wearable-preview": false });
    const { change } = stage(OUTFIT, { unavailable: <span data-testid="unavailable" /> });
    fromPreview({ type: "ready" });
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(cover()).toBeNull();

    change({ ...OUTFIT, urns: [HAT] });
    fromPreview({ type: "error", payload: { message: "not found" } });
    expect(frame()).toBeNull();

    change(OUTFIT);
    expect(frame()).not.toBeNull();
    expect(cover()).not.toBeNull();
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(cover()).toBeNull();
  });

  test("failed, then away to options that drew: back again it mounts and tries them once more", () => {
    primeRemoteFlags({ "dapps-unity-wearable-preview": false });
    const { change } = stage({ ...OUTFIT, urns: [HAT] }, { unavailable: <span data-testid="unavailable" /> });
    fromPreview({ type: "error", payload: { message: "not found" } });
    expect(frame()).toBeNull();

    change(OUTFIT);
    fromPreview({ type: "ready" });
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(cover()).toBeNull();

    change({ ...OUTFIT, urns: [HAT] });
    expect(frame()).not.toBeNull();
    expect(screen.queryByTestId("unavailable")).toBeNull();
    expect(cover()).not.toBeNull();
  });
});

describe("changes", () => {
  test("new items reload the scene in the same iframe, covered again until it draws", () => {
    primeRemoteFlags({ "dapps-unity-wearable-preview": false });
    const { change } = stage();
    const win = frame()!.contentWindow!;
    const posted = vi.spyOn(win, "postMessage");
    fromPreview({ type: "ready" });
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(cover()).toBeNull();

    change({ ...OUTFIT, urns: [HOODIE, HAT] });
    expect(frame()!.contentWindow).toBe(win);
    expect(posted.mock.calls.at(-1)![0]).toMatchObject({ type: "update", payload: { options: { urns: [HOODIE, HAT] } } });
    expect(cover()).not.toBeNull();
    fromPreview({ type: "load", payload: { renderer: "babylon" } });
    expect(cover()).toBeNull();
  });
});
