import { describe, expect, it } from "vitest";

import { homeDownloadActions } from "./actions";

const WINDOWS = { kind: "desktop", os: "windows", arch: "amd64" } as const;

describe("home download actions", () => {
  describe("when the deployment configures an installer host", () => {
    it("should start the visitor's installer and offer the other system", () => {
      const { hero, hangOut } = homeDownloadActions(WINDOWS, "+438K", true);
      expect(hero.primary?.href).toBe(
        "/download_success?os=windows&arch=amd64",
      );
      expect(hero.alternativeGroups?.[0]?.map((link) => link.href)).toEqual([
        "/download_success?os=macos&arch=arm64",
      ]);
      expect(
        hangOut.alternativeGroups?.flat().map((link) => link.icon),
      ).toEqual(["macos", "ios", "android"]);
    });

    it("should keep the download page link for 32-bit Windows", () => {
      expect(
        homeDownloadActions({ ...WINDOWS, arch: "ia32" }, "+438K", true).hero
          .primary?.href,
      ).toBe("/download");
    });
  });
  describe("when the visitor uses Android", () => {
    it("should offer only Google Play in the narrow hero", () => {
      expect(
        homeDownloadActions({ kind: "mobile", os: "android" }, "+438K", true),
      ).toMatchObject({
        store: { badge: "google_play" },
        alternateStore: undefined,
      });
    });
  });

  describe("when the visitor uses iOS", () => {
    it("should offer only the App Store in the narrow hero", () => {
      expect(
        homeDownloadActions({ kind: "mobile", os: "ios" }, "+438K", true),
      ).toMatchObject({
        store: { badge: "app_store" },
        alternateStore: undefined,
      });
    });
  });

  describe("when the mobile operating system is not identified", () => {
    it("should offer both stores in the narrow hero", () => {
      expect(
        homeDownloadActions({ kind: "unknown" }, "+438K", true),
      ).toMatchObject({
        store: { badge: "app_store" },
        alternateStore: { badge: "google_play" },
      });
    });
  });

  describe("when the deployment configures no installer host", () => {
    it("should lead to /download and offer no installer", () => {
      const { hero, hangOut } = homeDownloadActions(WINDOWS, "+438K", false);
      expect(hero.primary?.href).toBe("/download");
      expect(hangOut.primary?.href).toBe("/download");
      expect(hero.alternativeGroups?.[0]).toEqual([]);
    });
  });
});
