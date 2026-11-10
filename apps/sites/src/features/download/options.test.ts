import { describe, expect, it } from "vitest";

import { detectPlatform, downloadOffer, viewportOffers } from "./options";

const UA = {
  win64:
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
  winArm:
    "Mozilla/5.0 (Windows NT 10.0; ARM64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
  win32:
    "Mozilla/5.0 (Windows NT 10.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
  mac: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Safari/605.1.15",
  linux:
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
  android:
    "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36",
  iphone:
    "Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1",
};

const offer = (ua: string | null) => downloadOffer(detectPlatform(ua));

describe("downloadOffer", () => {
  it("gives a Mac the arm64 build first and Windows as the alternative, never an Intel Mac build", () => {
    expect(offer(UA.mac)).toEqual({
      primary: { os: "macos", arch: "arm64" },
      secondary: { os: "windows", arch: "amd64" },
      store: null,
    });
  });

  it("gives 64-bit Windows its installer and macOS as the alternative", () => {
    expect(offer(UA.win64)).toEqual({
      primary: { os: "windows", arch: "amd64" },
      secondary: { os: "macos", arch: "arm64" },
      store: null,
    });
  });

  it("gives Windows on ARM the amd64 installer, which runs emulated", () => {
    expect(offer(UA.winArm).primary).toEqual({ os: "windows", arch: "amd64" });
  });

  it("offers no installer to 32-bit Windows, which cannot run it", () => {
    expect(offer(UA.win32).primary).toBeNull();
  });

  it("gives Linux and an unknown visitor the Windows installer", () => {
    expect(offer(UA.linux).primary).toEqual({ os: "windows", arch: "amd64" });
    expect(offer(null)).toEqual({
      primary: { os: "windows", arch: "amd64" },
      secondary: { os: "macos", arch: "arm64" },
      store: null,
    });
  });

  it("gives phones one store badge and no installer", () => {
    expect(offer(UA.android)).toEqual({
      primary: null,
      secondary: null,
      store: "google_play",
    });
    expect(offer(UA.iphone)).toEqual({
      primary: null,
      secondary: null,
      store: "app_store",
    });
  });
});

describe("viewportOffers", () => {
  const offers = (ua: string) => viewportOffers(detectPlatform(ua));

  it("offers a desktop visitor its installers wide and the App Store narrow", () => {
    const mac = offers(
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
    );
    expect(mac.wide).toMatchObject({ primary: { os: "macos" }, store: null });
    expect(mac.narrowStore).toBe("app_store");
  });

  it("offers a phone the default installers wide and its own store narrow", () => {
    const android = offers(
      "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 Mobile",
    );
    expect(android.wide).toMatchObject({
      primary: { os: "windows" },
      secondary: { os: "macos" },
      store: null,
    });
    expect(android.narrowStore).toBe("google_play");
    expect(
      offers("Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X)")
        .narrowStore,
    ).toBe("app_store");
  });
});
