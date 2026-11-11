import { describe, expect, it } from "vitest";
import { downloadActions } from "./actions";
import { detectPlatform } from "./options";

describe("download action links", () => {
  it.each([
    ["Windows NT 10.0; Win64; x64", "windows", ".exe"],
    ["Macintosh; Intel Mac OS X", "macos", ".dmg"],
    ["", "windows", ".exe"],
    ["Linux x86_64", "windows", ".exe"],
  ])("should offer a direct pinned installer for %s", (ua, os, suffix) => {
    const { actions } = downloadActions(detectPlatform(ua), "+438K");
    expect(actions.primary?.os).toBe(os);
    expect(actions.primary?.href).toBe(
      `https://explorer-artifacts.decentraland.org/launcher-rust/Decentraland_installer${suffix}`,
    );
    expect(actions.primary?.href).not.toContain("/download_success");
    expect(actions.downloads).toBe("Total Downloads: +438K");
  });
  it("should omit the Windows installer for a 32-bit Windows visitor", () => {
    const { actions } = downloadActions(
      detectPlatform("Windows NT 6.1"),
      "+400K",
    );
    expect(actions.primary).toBeUndefined();
    expect(actions.alternatives?.[0]?.href).toMatch(/\.dmg$/);
  });
  it.each([
    ["Android", "google_play", "play.google.com"],
    ["iPhone", "app_store", "apps.apple.com"],
  ])("should use the appropriate mobile store for %s", (ua, badge, host) => {
    const { store } = downloadActions(detectPlatform(ua), "+400K");
    expect(store.badge).toBe(badge);
    expect(new URL(store.href).host).toBe(host);
  });
});
