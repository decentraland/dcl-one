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
    it("should offer Google Play before the App Store in the hero", () => {
      const { store, alternateStore } = homeDownloadActions(
        { kind: "mobile", os: "android" },
        "+438K",
        true,
      );
      expect([store.badge, alternateStore.badge]).toEqual([
        "google_play",
        "app_store",
      ]);
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
