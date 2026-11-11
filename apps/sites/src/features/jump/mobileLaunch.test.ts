// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { STORES } from "../download/actions";
import { androidIntentUrl, jumpInFor, mobileDeepLink, mobileLaunch, openOnIos } from "./mobileLaunch";

const ANDROID = { kind: "mobile", os: "android" } as const;
const IOS = { kind: "mobile", os: "ios" } as const;
const DESKTOP = { kind: "desktop", os: "macos", arch: "unknown" } as const;

describe("mobileDeepLink", () => {
  it("should carry the parcel and World to the app's open route", () => {
    expect(mobileDeepLink({ coordinates: [-3, 2], realm: "foo.dcl.eth" })).toBe(
      "decentraland://open?position=-3%2C2&realm=foo.dcl.eth",
    );
  });

  it("should drop the defaults the app already uses", () => {
    expect(mobileDeepLink({ coordinates: [0, 0], realm: "main" })).toBe("decentraland://open");
  });

  it("should drop a realm that is not a realm name", () => {
    expect(mobileDeepLink({ coordinates: [1, 1], realm: "x\"><script>" })).toBe("decentraland://open?position=1%2C1");
  });
});

describe("androidIntentUrl", () => {
  it("should name the package and the store fallback", () => {
    expect(androidIntentUrl("decentraland://open?position=1%2C1", "https://store.example/app?id=1")).toBe(
      "intent://open?position=1%2C1#Intent;scheme=decentraland;package=org.decentraland.godotexplorer;S.browser_fallback_url=https%3A%2F%2Fstore.example%2Fapp%3Fid%3D1;end",
    );
  });
});

describe("mobileLaunch", () => {
  const target = { coordinates: [5, 6] as [number, number] };

  it("should leave desktop visitors on the web explorer", () => {
    expect(mobileLaunch(DESKTOP, target)).toBeNull();
    expect(mobileLaunch({ kind: "unknown" }, target)).toBeNull();
  });

  it("should send Android through an intent with the Play Store fallback", () => {
    expect(mobileLaunch(ANDROID, target)).toEqual({
      href: androidIntentUrl("decentraland://open?position=5%2C6", STORES.google_play),
      storeUrl: STORES.google_play,
      android: true,
    });
  });

  it("should send iOS through the deep link with the App Store as its fallback", () => {
    expect(mobileLaunch(IOS, target)).toEqual({
      href: "decentraland://open?position=5%2C6",
      storeUrl: STORES.app_store,
      android: false,
    });
  });
});

describe("jumpInFor", () => {
  it("should offer nothing when the card has no Jump In", () => {
    expect(jumpInFor(undefined, mobileLaunch(IOS, { coordinates: [1, 1] }))).toEqual({});
  });

  it("should keep the web explorer link on desktop", () => {
    expect(jumpInFor("/play/?position=1%2C1", null)).toEqual({ jumpInHref: "/play/?position=1%2C1" });
  });

  it("should link Android straight to the intent, with no click handler", () => {
    const launch = mobileLaunch(ANDROID, { coordinates: [1, 1] })!;
    expect(jumpInFor("/play/", launch)).toEqual({ jumpInHref: launch.href });
  });
});

describe("openOnIos", () => {
  const assign = vi.fn();
  const launch = { href: "decentraland://open", storeUrl: "https://apps.example/dcl", android: false };

  beforeEach(() => {
    vi.useFakeTimers();
    assign.mockReset();
    Object.defineProperty(window, "location", { value: { assign }, configurable: true });
  });
  afterEach(() => vi.useRealTimers());

  it.each([
    ["blur", () => window.dispatchEvent(new Event("blur"))],
    ["pagehide", () => window.dispatchEvent(new Event("pagehide"))],
    [
      "visibilitychange to hidden",
      () => {
        Object.defineProperty(document, "hidden", { value: true, configurable: true });
        document.dispatchEvent(new Event("visibilitychange"));
        Object.defineProperty(document, "hidden", { value: false, configurable: true });
      },
    ],
  ])("should stay put when the app takes the link (%s)", async (_signal, leave) => {
    const opened = openOnIos(launch, 1500);
    leave();
    await vi.advanceTimersByTimeAsync(1500);
    await expect(opened).resolves.toBe(true);
    expect(assign.mock.calls).toEqual([["decentraland://open"]]);
  });

  it("should open the App Store when the page never loses focus", async () => {
    const opened = openOnIos(launch, 1500);
    await vi.advanceTimersByTimeAsync(1500);
    await expect(opened).resolves.toBe(false);
    expect(assign.mock.calls).toEqual([["decentraland://open"], ["https://apps.example/dcl"]]);
  });

  it("should open the App Store at once when the scheme throws", async () => {
    assign.mockImplementationOnce(() => {
      throw new Error("blocked");
    });
    await expect(openOnIos(launch, 1500)).resolves.toBe(false);
    expect(assign).toHaveBeenLastCalledWith("https://apps.example/dcl");
  });
});
