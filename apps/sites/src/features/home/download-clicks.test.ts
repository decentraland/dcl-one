import type { MouseEvent } from "react";
import { describe, expect, it, vi } from "vitest";

import type { DownloadAlternative } from "@dcl/one-ui/apps/sites/download/DownloadActions";

import { detectPlatform } from "../download/options";
import type { DownloadClick } from "../download/download-click";
import { homeDownloadActions } from "./actions";
import { withDownloadClicks } from "./download-clicks";

const MAC = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/120 Safari/537.36";
const WINDOWS = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120 Safari/537.36";
const ANDROID = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 Chrome/120 Mobile Safari/537.36";
const IPHONE = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 Mobile/15E148";

const event = {} as MouseEvent<HTMLElement>;

function home(ua: string) {
  const send = vi.fn<(click: DownloadClick) => void>();
  const actions = withDownloadClicks(homeDownloadActions(detectPlatform(ua), "+400K", true), send);
  const click = (onClick?: (e: MouseEvent<HTMLElement>) => void): DownloadClick => {
    send.mockClear();
    onClick!(event);
    expect(send).toHaveBeenCalledTimes(1);
    return send.mock.calls[0]![0];
  };
  const alt = (groups: readonly (readonly DownloadAlternative[])[] | undefined, icon: DownloadAlternative["icon"]) =>
    groups!.flat().find((a) => a.icon === icon)!.onClick;
  return { actions, click, alt };
}

describe("home download clicks (legacy inventory 2.1.a)", () => {
  it("row 1: hero Android store badge", () => {
    const { actions, click } = home(ANDROID);
    expect(click(actions.store.onClick)).toEqual({ place: "Landing Hero", event: "Download", download_target: "google_play", os: "Android" });
  });

  it("row 2: hero App Store badge", () => {
    const { actions, click } = home(IPHONE);
    expect(click(actions.store.onClick)).toEqual({ place: "Landing Hero", event: "Download", download_target: "app_store", os: "iOS" });
  });

  it("row 1 (narrow desktop): the hero's alternate Google Play badge", () => {
    const { actions, click } = home(MAC);
    expect(click(actions.alternateStore!.onClick)).toEqual({
      place: "Landing Hero",
      event: "Download",
      download_target: "google_play",
      os: "Android",
    });
  });

  it("row 3: hero main installer button carries no os", () => {
    const { actions, click } = home(MAC);
    expect(click(actions.hero.primary!.onClick)).toEqual({ place: "Landing Hero", event: "Download", download_target: "desktop_installer" });
  });

  it("row 4: hero Epic button", () => {
    const { actions, click } = home(WINDOWS);
    expect(click(actions.hero.epic!.onClick)).toEqual({ place: "landing-hero-epic", event: "Download", download_target: "epic" });
  });

  it("row 5: hero Windows switch shown on macOS", () => {
    const { actions, click, alt } = home(MAC);
    expect(click(alt(actions.hero.alternativeGroups, "windows"))).toEqual({
      place: "landing-hero-platform-switch",
      event: "Download",
      download_target: "desktop_installer",
      os: "Windows",
    });
  });

  it("row 6: hero macOS switch shown on Windows", () => {
    const { actions, click, alt } = home(WINDOWS);
    expect(click(alt(actions.hero.alternativeGroups, "macos"))).toEqual({
      place: "landing-hero-platform-switch",
      event: "Download",
      download_target: "desktop_installer",
      os: "macOS",
    });
  });

  it("row 7: hero iOS icon", () => {
    const { actions, click, alt } = home(WINDOWS);
    expect(click(alt(actions.hero.alternativeGroups, "ios"))).toEqual({
      place: "landing-hero-platform-switch",
      event: "Download",
      download_target: "app_store",
      os: "iOS",
    });
  });

  it("row 8: hero Android icon", () => {
    const { actions, click, alt } = home(WINDOWS);
    expect(click(alt(actions.hero.alternativeGroups, "android"))).toEqual({
      place: "landing-hero-platform-switch",
      event: "Download",
      download_target: "google_play",
      os: "Android",
    });
  });

  it("row 9: Come Hang Out Android store badge", () => {
    const { actions, click } = home(ANDROID);
    expect(click(actions.hangOutStore.onClick)).toEqual({
      place: "Landing Come Hang Out",
      event: "Download",
      download_target: "google_play",
      os: "Android",
    });
  });

  it("row 10: Come Hang Out App Store badge", () => {
    const { actions, click } = home(IPHONE);
    expect(click(actions.hangOutStore.onClick)).toEqual({
      place: "Landing Come Hang Out",
      event: "Download",
      download_target: "app_store",
      os: "iOS",
    });
  });

  it("row 11: Come Hang Out main button sends no event property", () => {
    const { actions, click } = home(MAC);
    const sent = click(actions.hangOut.primary!.onClick);
    expect(sent).toEqual({ place: "come-hang-out", download_target: "desktop_installer" });
    expect(sent).not.toHaveProperty("event");
  });

  it("row 12: Come Hang Out Epic button", () => {
    const { actions, click } = home(MAC);
    expect(click(actions.hangOut.epic!.onClick)).toEqual({ place: "come-hang-out", event: "Download", download_target: "epic" });
  });

  it("row 13: Come Hang Out Windows switch on macOS", () => {
    const { actions, click, alt } = home(MAC);
    expect(click(alt(actions.hangOut.alternativeGroups, "windows"))).toEqual({
      place: "come-hang-out-platform-switch",
      event: "Download",
      download_target: "desktop_installer",
      os: "Windows",
    });
  });

  it("row 14: Come Hang Out macOS switch on Windows", () => {
    const { actions, click, alt } = home(WINDOWS);
    expect(click(alt(actions.hangOut.alternativeGroups, "macos"))).toEqual({
      place: "come-hang-out-platform-switch",
      event: "Download",
      download_target: "desktop_installer",
      os: "macOS",
    });
  });

  it("row 15: Come Hang Out iOS icon", () => {
    const { actions, click, alt } = home(MAC);
    expect(click(alt(actions.hangOut.alternativeGroups, "ios"))).toEqual({
      place: "come-hang-out-platform-switch",
      event: "Download",
      download_target: "app_store",
      os: "iOS",
    });
  });

  it("row 16: Come Hang Out Android icon", () => {
    const { actions, click, alt } = home(MAC);
    expect(click(alt(actions.hangOut.alternativeGroups, "android"))).toEqual({
      place: "come-hang-out-platform-switch",
      event: "Download",
      download_target: "google_play",
      os: "Android",
    });
  });

  it("keeps every store and Epic link as the actions gave them", () => {
    const plain = homeDownloadActions(detectPlatform(MAC), "+400K", true);
    const { actions } = home(MAC);
    expect(actions.hero.epic!.href).toBe(plain.hero.epic!.href);
    expect(actions.hero.alternativeGroups!.flat().filter((a) => a.icon === "ios" || a.icon === "android").map((a) => a.href)).toEqual(
      plain.hero.alternativeGroups!.flat().filter((a) => a.icon === "ios" || a.icon === "android").map((a) => a.href),
    );
    expect(actions.hangOutStore.href).toBe(plain.store.href);
  });

  it("adds no alternate badge where the actions have none (a phone)", () => {
    expect(home(ANDROID).actions.alternateStore).toBeUndefined();
  });
});

describe("home /download_success links (legacy inventory 4.3)", () => {
  const ANON = "0b7f4c2e-1d3a-4e5f-9a8b-7c6d5e4f3a2b";
  const link = (ua: string, ctx?: Parameters<typeof withDownloadClicks>[2]) =>
    withDownloadClicks(homeDownloadActions(detectPlatform(ua), "+400K", true), vi.fn(), ctx);

  it("puts each section's place on its main button, with no architecture, as the legacy hero did", () => {
    const actions = link(MAC);
    expect(actions.hero.primary!.href).toBe("/download_success?os=macOS&place=landing-hero");
    expect(actions.hangOut.primary!.href).toBe("/download_success?os=macOS&place=come-hang-out");
  });

  it("puts the platform-switch place on the other installer", () => {
    const actions = link(MAC);
    const other = (groups: typeof actions.hero.alternativeGroups) => groups!.flat().find((a) => a.icon === "windows")!.href;
    expect(other(actions.hero.alternativeGroups)).toBe("/download_success?os=Windows&place=landing-hero-platform-switch");
    expect(other(actions.hangOut.alternativeGroups)).toBe("/download_success?os=Windows&place=come-hang-out-platform-switch");
  });

  it("adds the visitor's anonymous id, then the page's utm_*, in the legacy order", () => {
    const actions = link(WINDOWS, { anonUserId: ANON, search: "utm_source=Partner%20X&utm_campaign=fall" });
    expect(actions.hero.primary!.href).toBe(
      `/download_success?os=Windows&place=landing-hero&anon_user_id=${ANON}&utm_source=partner_x&utm_campaign=fall`,
    );
  });

  it("keeps the page's utm_* on the /download fallback and leaves it bare without them", () => {
    const withoutInstallers = (search: string) =>
      withDownloadClicks(homeDownloadActions(detectPlatform(WINDOWS), "+400K", false), vi.fn(), { search }).hero.primary!.href;
    expect(withoutInstallers("utm_source=x")).toBe("/download?utm_source=x");
    expect(withoutInstallers("")).toBe("/download");
  });
});
