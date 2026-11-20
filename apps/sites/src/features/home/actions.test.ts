import { describe, expect, it } from "vitest";

import type { DetectedPlatform } from "../download/options";
import { homeDownloadActions } from "./actions";

const WINDOWS = { kind: "desktop", os: "windows", arch: "amd64" } as const;
const MACOS = { kind: "desktop", os: "macos", arch: "unknown" } as const;
const LINUX = { kind: "desktop", os: "other", arch: "unknown" } as const;
const UNKNOWN = { kind: "unknown" } as const;
const ANDROID = { kind: "mobile", os: "android" } as const;

const primaryOf = (platform: DetectedPlatform, installers: boolean) => {
  const { hero, hangOut } = homeDownloadActions(platform, "+438K", installers);
  expect(hangOut.primary).toEqual(hero.primary);
  return hero.primary;
};

describe("home download actions", () => {
  describe("when the deployment configures an installer host", () => {
    it("should start the visitor's installer and offer the other system", () => {
      const { hero, hangOut } = homeDownloadActions(WINDOWS, "+438K", true);
      expect(hero.primary).toMatchObject({
        os: "windows",
        osName: "Windows",
        href: "/download_success?os=windows&arch=amd64",
      });
      expect(hero.alternativeGroups?.[0]?.map((link) => link.href)).toEqual([
        "/download_success?os=macos&arch=arm64",
      ]);
      expect(
        hangOut.alternativeGroups?.flat().map((link) => link.icon),
      ).toEqual(["macos", "ios", "android"]);
    });

    it("should start the Apple installer for a Mac visitor and offer Windows", () => {
      const { hero } = homeDownloadActions(MACOS, "+438K", true);
      expect(hero.primary).toMatchObject({
        os: "macos",
        osName: "macOS",
        href: "/download_success?os=macos&arch=arm64",
      });
      expect(hero.alternativeGroups?.[0]?.map((link) => link.icon)).toEqual([
        "windows",
      ]);
    });

    it.each([
      ["a Linux desktop", LINUX],
      ["an unknown platform", UNKNOWN],
    ])(
      "should default %s to the Windows icon and installer and offer macOS",
      (_, platform) => {
        const { hero } = homeDownloadActions(platform, "+438K", true);
        expect(hero.primary).toMatchObject({
          os: "windows",
          osName: "Windows",
          href: "/download_success?os=windows&arch=amd64",
        });
        expect(hero.alternativeGroups?.[0]?.map((link) => link.icon)).toEqual(
          ["macos"],
        );
      },
    );

    it("should keep the Windows icon and the download page link for 32-bit Windows", () => {
      const { hero } = homeDownloadActions(
        { ...WINDOWS, arch: "ia32" },
        "+438K",
        true,
      );
      expect(hero.primary).toMatchObject({ os: "windows", href: "/download" });
      expect(hero.alternativeGroups?.[0]).toEqual([]);
    });
  });

  describe("when the visitor uses Android", () => {
    it("should offer Google Play before the App Store in the hero", () => {
      const { store, alternateStore } = homeDownloadActions(
        ANDROID,
        "+438K",
        true,
      );
      expect([store.badge, alternateStore.badge]).toEqual([
        "google_play",
        "app_store",
      ]);
    });

    it("should keep the download page link, now with the Windows icon, and no installer", () => {
      const { hero } = homeDownloadActions(ANDROID, "+438K", true);
      expect(hero.primary).toMatchObject({ os: "windows", href: "/download" });
      expect(hero.alternativeGroups?.[0]).toEqual([]);
    });
  });

  describe("when the deployment configures no installer host", () => {
    it("should lead to /download and offer no installer", () => {
      const { hero, hangOut } = homeDownloadActions(WINDOWS, "+438K", false);
      expect(hero.primary?.href).toBe("/download");
      expect(hangOut.primary?.href).toBe("/download");
      expect(hero.alternativeGroups?.[0]).toEqual([]);
      expect(hangOut.alternativeGroups?.flat().map((link) => link.icon)).toEqual(
        ["ios", "android"],
      );
    });

    it.each([
      ["a Mac visitor", MACOS, "macos", "macOS"],
      ["a Windows visitor", WINDOWS, "windows", "Windows"],
      ["a Linux desktop", LINUX, "windows", "Windows"],
      ["an unknown platform", UNKNOWN, "windows", "Windows"],
    ] as const)("should still show the OS icon for %s", (_, platform, os, name) => {
      expect(primaryOf(platform, false)).toEqual({
        label: "DOWNLOAD FOR",
        os,
        osName: name,
        href: "/download",
      });
    });
  });
});
