import { describe, expect, it } from "vitest";

import { detectPlatform, downloadOffer, viewportOffers } from "./options";

describe("download platform selection", () => {
  describe("when detecting the request platform", () => {
    it("should select the Windows amd64 installer for a 64-bit Windows browser", () => {
      expect(detectPlatform("Windows NT 10.0; Win64; x64")).toEqual({
        kind: "desktop",
        os: "windows",
        arch: "amd64",
      });
    });

    it("should identify a Mac browser without a reliable architecture", () => {
      expect(detectPlatform("Macintosh; Intel Mac OS X 10_15_7")).toEqual({
        kind: "desktop",
        os: "macos",
        arch: "unknown",
      });
    });

    it("should use the iOS store for an iPhone browser", () => {
      expect(
        detectPlatform("iPhone; CPU iPhone OS 18_0 like Mac OS X"),
      ).toEqual({
        kind: "mobile",
        os: "ios",
      });
    });

    it("should use the Android store for an Android browser", () => {
      expect(detectPlatform("Android 15; Pixel 9")).toEqual({
        kind: "mobile",
        os: "android",
      });
    });

    it("should safely fall back for a missing user agent", () => {
      expect(detectPlatform(null)).toEqual({ kind: "unknown" });
    });
  });

  describe("when offering installers", () => {
    it("should not offer a Windows installer to a 32-bit Windows browser", () => {
      expect(downloadOffer(detectPlatform("Windows NT 6.1"))).toEqual({
        primary: null,
        secondary: { os: "macos", arch: "arm64" },
        store: null,
      });
    });

    it("should use the Android store on a narrow Android layout", () => {
      expect(viewportOffers(detectPlatform("Android 15; Pixel 9"))).toEqual({
        wide: {
          primary: { os: "windows", arch: "amd64" },
          secondary: { os: "macos", arch: "arm64" },
          store: null,
        },
        narrowStore: "google_play",
      });
    });
  });
});
