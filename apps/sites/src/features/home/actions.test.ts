import { describe, expect, it } from "vitest";

import { homeDownloadActions } from "./actions";

const WINDOWS = { kind: "desktop", os: "windows", arch: "amd64" } as const;

describe("home download actions", () => {
  describe("when the deployment configures an installer host", () => {
    it("should start the visitor's installer and offer the other system", () => {
      const { hero } = homeDownloadActions(WINDOWS, "+438K", true);
      expect(hero.primary?.href).toBe("/download_success?os=windows&arch=amd64");
      expect(hero.alternativeGroups?.[0]?.map((link) => link.href)).toEqual(["/download_success?os=macos&arch=arm64"]);
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
