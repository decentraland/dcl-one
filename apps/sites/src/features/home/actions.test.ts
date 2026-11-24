import { describe, expect, it } from "vitest";

import type { DetectedPlatform } from "../download/options";
import { googlePlayHref } from "../download/google-play";
import { homeDownloadActions } from "./actions";

const WINDOWS = { kind: "desktop", os: "windows", arch: "amd64" } as const;
const MACOS = { kind: "desktop", os: "macos", arch: "unknown" } as const;
const LINUX = { kind: "desktop", os: "other", arch: "unknown" } as const;
const UNKNOWN = { kind: "unknown" } as const;
const ANDROID = { kind: "mobile", os: "android" } as const;
const IOS = { kind: "mobile", os: "ios" } as const;
const WINDOWS_ARM = { ...WINDOWS, arch: "arm64" } as const;
const WINDOWS_32 = { ...WINDOWS, arch: "ia32" } as const;

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
        expect(hero.alternativeGroups?.[0]?.map((link) => link.icon)).toEqual([
          "macos",
        ]);
      },
    );

    it("should start the 64-bit Windows installer for a Windows arm64 visitor", () => {
      expect(primaryOf(WINDOWS_ARM, true)).toMatchObject({
        os: "windows",
        href: "/download_success?os=windows&arch=amd64",
      });
    });

    it("should keep the Windows icon, link 32-bit Windows to the download page and still offer macOS", () => {
      const { hero, hangOut } = homeDownloadActions(WINDOWS_32, "+438K", true);
      expect(primaryOf(WINDOWS_32, true)).toEqual({
        label: "DOWNLOAD FOR",
        os: "windows",
        osName: "Windows",
        href: "/download",
      });
      expect(hero.alternativeGroups?.[0]?.map((link) => link.href)).toEqual([
        "/download_success?os=macos&arch=arm64",
      ]);
      expect(
        hangOut.alternativeGroups?.flat().map((link) => link.icon),
      ).toEqual(["macos", "ios", "android"]);
    });

    it.each([
      ["a Mac visitor", MACOS],
      ["a Windows visitor", WINDOWS],
      ["a Linux desktop", LINUX],
      ["an unknown platform", UNKNOWN],
      ["32-bit Windows", WINDOWS_32],
      ["an Android phone", ANDROID],
    ] as const)(
      "should give the hero and Come Hang Out the same call and links for %s",
      (_, platform) => {
        const { hero, hangOut } = homeDownloadActions(platform, "+438K", true);
        expect(primaryOf(platform, true)?.os).toMatch(/^(windows|macos)$/);
        expect(hangOut.alternativeGroups?.flat()).toEqual(
          hero.alternativeGroups?.flat(),
        );
      },
    );
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

    it("should link to the download page with the Windows icon and no installer", () => {
      const { hero } = homeDownloadActions(ANDROID, "+438K", true);
      expect(hero.primary).toMatchObject({ os: "windows", href: "/download" });
      expect(hero.alternativeGroups?.[0]).toEqual([]);
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

    it("should link to the download page with the Windows icon and no installer", () => {
      const { hero } = homeDownloadActions(IOS, "+438K", true);
      expect(primaryOf(IOS, true)).toMatchObject({
        os: "windows",
        osName: "Windows",
        href: "/download",
      });
      expect(hero.alternativeGroups?.[0]).toEqual([]);
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
      expect(
        hangOut.alternativeGroups?.flat().map((link) => link.icon),
      ).toEqual(["ios", "android"]);
    });

    it.each([
      ["a Mac visitor", MACOS, "macos", "macOS"],
      ["a Windows visitor", WINDOWS, "windows", "Windows"],
      ["a Linux desktop", LINUX, "windows", "Windows"],
      ["an unknown platform", UNKNOWN, "windows", "Windows"],
      ["32-bit Windows", WINDOWS_32, "windows", "Windows"],
      ["an iPhone", IOS, "windows", "Windows"],
    ] as const)(
      "should still show the OS icon for %s",
      (_, platform, os, name) => {
        expect(primaryOf(platform, false)).toEqual({
          label: "DOWNLOAD FOR",
          os,
          osName: name,
          href: "/download",
        });
      },
    );
  });
});

describe("Google Play links", () => {
  const tagged = googlePlayHref("?utm_source=Partner");
  const googlePlayHrefs = (platform: DetectedPlatform, href?: string) => {
    const { hero, hangOut, store, alternateStore } = homeDownloadActions(platform, "+438K", true, undefined, href);
    return [
      ...[hero, hangOut].flatMap((a) => a.alternativeGroups!.flat().filter((alt) => alt.icon === "android").map((alt) => alt.href)),
      ...[store, alternateStore].filter((s) => s?.badge === "google_play").map((s) => s!.href),
    ];
  };

  it("carries the visitor's campaign on every Google Play control", () => {
    expect(googlePlayHrefs(ANDROID, tagged)).toEqual([tagged, tagged, tagged]);
    expect(googlePlayHrefs(WINDOWS, tagged)).toEqual([tagged, tagged, tagged]);
    expect(new URL(tagged).searchParams.get("utm_source")).toBe("partner");
  });

  it("uses the site's own tag by default", () => {
    expect(new Set(googlePlayHrefs(WINDOWS))).toEqual(new Set([googlePlayHref()]));
  });
});
