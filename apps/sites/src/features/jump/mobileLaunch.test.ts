// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from "vitest";

import { STORES } from "../download/actions";
import { androidIntentUrl, jumpInFor, type MobileLaunch, mobileDeepLink, mobileLaunch, openOnIos } from "./mobileLaunch";

const ANDROID = { kind: "mobile", os: "android" } as const;
const IOS = { kind: "mobile", os: "ios" } as const;
const DESKTOP = { kind: "desktop", os: "macos", arch: "unknown" } as const;

describe("mobileDeepLink", () => {
  describe("when the target names a parcel and a World", () => {
    let target: { coordinates: [number, number]; realm?: string };

    beforeEach(() => {
      target = { coordinates: [-3, 2], realm: "foo.dcl.eth" };
    });

    it("should carry the parcel and World to the app's open route", () => {
      expect(mobileDeepLink(target)).toBe("decentraland://open?position=-3%2C2&realm=foo.dcl.eth");
    });
  });

  describe("when the target is the app's defaults", () => {
    let target: { coordinates: [number, number]; realm?: string };

    beforeEach(() => {
      target = { coordinates: [0, 0], realm: "main" };
    });

    it("should drop the defaults the app already uses", () => {
      expect(mobileDeepLink(target)).toBe("decentraland://open");
    });
  });

  describe("when the realm is not a realm name", () => {
    let target: { coordinates: [number, number]; realm?: string };

    beforeEach(() => {
      target = { coordinates: [1, 1], realm: "x\"><script>" };
    });

    it("should drop the realm", () => {
      expect(mobileDeepLink(target)).toBe("decentraland://open?position=1%2C1");
    });
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
  let target: { coordinates: [number, number] };

  beforeEach(() => {
    target = { coordinates: [5, 6] };
  });

  describe("when the visitor is on desktop", () => {
    it("should leave them on the web explorer", () => {
      expect(mobileLaunch(DESKTOP, target)).toBeNull();
    });
  });

  describe("when the visitor's platform is unknown", () => {
    it("should leave them on the web explorer", () => {
      expect(mobileLaunch({ kind: "unknown" }, target)).toBeNull();
    });
  });

  describe("when the visitor is on Android", () => {
    it("should send them through an intent with the Play Store fallback", () => {
      expect(mobileLaunch(ANDROID, target)).toEqual({
        href: androidIntentUrl("decentraland://open?position=5%2C6", STORES.google_play),
        storeUrl: STORES.google_play,
        android: true,
      });
    });
  });

  describe("when the visitor is on iOS", () => {
    it("should send them through the deep link with the App Store as its fallback", () => {
      expect(mobileLaunch(IOS, target)).toEqual({
        href: "decentraland://open?position=5%2C6",
        storeUrl: STORES.app_store,
        android: false,
      });
    });
  });
});

describe("jumpInFor", () => {
  describe("when the card has no Jump In", () => {
    let launch: MobileLaunch | null;

    beforeEach(() => {
      launch = mobileLaunch(IOS, { coordinates: [1, 1] });
    });

    it("should offer nothing", () => {
      expect(jumpInFor(undefined, launch)).toEqual({});
    });
  });

  describe("when the visitor is on desktop", () => {
    it("should keep the web explorer link", () => {
      expect(jumpInFor("/play/?position=1%2C1", null)).toEqual({ jumpInHref: "/play/?position=1%2C1" });
    });
  });

  describe("when the visitor is on Android", () => {
    let launch: MobileLaunch;

    beforeEach(() => {
      launch = mobileLaunch(ANDROID, { coordinates: [1, 1] })!;
    });

    it("should link straight to the intent, with no click handler", () => {
      expect(jumpInFor("/play/", launch)).toEqual({ jumpInHref: launch.href });
    });
  });
});

describe("openOnIos", () => {
  let assign: Mock;
  let launch: MobileLaunch;

  beforeEach(() => {
    vi.useFakeTimers();
    assign = vi.fn();
    launch = { href: "decentraland://open", storeUrl: "https://apps.example/dcl", android: false };
    Object.defineProperty(window, "location", { value: { assign }, configurable: true });
  });

  afterEach(() => {
    vi.resetAllMocks();
    vi.useRealTimers();
  });

  describe.each([
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
  ])("when the app takes the link (%s)", (_signal, leave) => {
    let opened: Promise<boolean>;

    beforeEach(async () => {
      opened = openOnIos(launch, 1500);
      leave();
      await vi.advanceTimersByTimeAsync(1500);
    });

    it("should resolve as opened", async () => {
      await expect(opened).resolves.toBe(true);
    });

    it("should stay put after following the deep link", () => {
      expect(assign.mock.calls).toEqual([["decentraland://open"]]);
    });
  });

  describe("when the page never loses focus", () => {
    let opened: Promise<boolean>;

    beforeEach(async () => {
      opened = openOnIos(launch, 1500);
      await vi.advanceTimersByTimeAsync(1500);
    });

    it("should resolve as not opened", async () => {
      await expect(opened).resolves.toBe(false);
    });

    it("should open the App Store after the deep link", () => {
      expect(assign.mock.calls).toEqual([["decentraland://open"], ["https://apps.example/dcl"]]);
    });
  });

  describe("when the scheme throws", () => {
    let opened: Promise<boolean>;

    beforeEach(() => {
      assign.mockImplementationOnce(() => {
        throw new Error("blocked");
      });
      opened = openOnIos(launch, 1500);
    });

    it("should resolve as not opened", async () => {
      await expect(opened).resolves.toBe(false);
    });

    it("should open the App Store at once", async () => {
      await opened;
      expect(assign).toHaveBeenLastCalledWith("https://apps.example/dcl");
    });
  });
});
