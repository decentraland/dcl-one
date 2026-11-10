import { describe, expect, it, vi } from "vitest";

import { downloadActionsProps, storeBadgeProps } from "./action-props";
import { downloadOffer } from "./options";

const i18n = { t: (key: string) => key, text: (key: string) => key };
const places = {
  installer: "landing-hero",
  installerClick: "Landing Hero",
  platformSwitch: "landing-hero-platform-switch",
  epic: "landing-hero-epic",
  store: "Landing Hero",
} as const;

function ctas() {
  return {
    installerHref: vi.fn(
      (option: { os: string }, place: string) =>
        `/download_success?os=${option.os}&place=${place}`,
    ),
    chooseInstaller: vi.fn(
      (option: { os: string }, place: string) => `/go/${option.os}/${place}`,
    ),
    leaveForStore: vi.fn(),
  };
}

const mac = downloadOffer({ kind: "desktop", os: "macos", arch: "unknown" });

describe("downloadActionsProps", () => {
  it("links the primary installer and the platform switch with their own places", () => {
    const props = downloadActionsProps({
      offer: mac,
      ctas: ctas(),
      i18n,
      places,
      downloadsLabel: "+400K",
      campaign: {},
    });

    expect(props.primary).toMatchObject({
      os: "macos",
      href: "/download_success?os=macos&place=landing-hero",
    });
    expect(props.alternatives?.[0]).toMatchObject({
      icon: "windows",
      href: "/download_success?os=windows&place=landing-hero-platform-switch",
    });
  });

  it("navigates through chooseInstaller so the click is recorded first", () => {
    const c = ctas();
    const assign = vi.fn();
    vi.stubGlobal("window", { location: { assign } });
    const props = downloadActionsProps({
      offer: mac,
      ctas: c,
      i18n,
      places,
      downloadsLabel: "x",
      campaign: {},
    });
    const preventDefault = vi.fn();

    props.primary!.onClick({ preventDefault } as never);

    expect(preventDefault).toHaveBeenCalled();
    expect(assign).toHaveBeenCalledWith("/go/macos/landing-hero");
    // The Click names the section as source does; the URL keeps the DownloadPlace.
    expect(c.chooseInstaller).toHaveBeenCalledWith(
      { os: "macos", arch: "arm64" },
      "landing-hero",
      "Landing Hero",
    );
    vi.unstubAllGlobals();
  });

  it.each([
    ["Cmd", { metaKey: true }],
    ["Ctrl", { ctrlKey: true }],
    ["Shift", { shiftKey: true }],
    ["Alt", { altKey: true }],
    ["middle button", { button: 1 }],
    ["already claimed", { defaultPrevented: true }],
  ])(
    "records a click (%s) on the primary and alternate installer but leaves it to the browser",
    (_, modifier) => {
      const c = ctas();
      const assign = vi.fn();
      vi.stubGlobal("window", { location: { assign } });
      const props = downloadActionsProps({
        offer: mac,
        ctas: c,
        i18n,
        places,
        downloadsLabel: "x",
        campaign: {},
      });

      for (const link of [props.primary!, props.alternatives![0]!]) {
        const preventDefault = vi.fn();
        link.onClick({ preventDefault, button: 0, ...modifier } as never);
        expect(preventDefault).not.toHaveBeenCalled();
      }
      expect(c.chooseInstaller).toHaveBeenCalledTimes(2);
      expect(assign).not.toHaveBeenCalled();
      vi.unstubAllGlobals();
    },
  );

  it("takes over a plain click on the alternate installer too", () => {
    const assign = vi.fn();
    vi.stubGlobal("window", { location: { assign } });
    const props = downloadActionsProps({
      offer: mac,
      ctas: ctas(),
      i18n,
      places,
      downloadsLabel: "x",
      campaign: {},
    });
    const preventDefault = vi.fn();

    props.alternatives![0]!.onClick({ preventDefault, button: 0 } as never);

    expect(preventDefault).toHaveBeenCalled();
    expect(assign).toHaveBeenCalledWith(
      "/go/windows/landing-hero-platform-switch",
    );
    vi.unstubAllGlobals();
  });

  it("links phone platforms to the stores, or opens the QR dialog when given one", () => {
    const links = downloadActionsProps({
      offer: mac,
      ctas: ctas(),
      i18n,
      places,
      downloadsLabel: "x",
      campaign: {},
    });
    expect(
      links.alternatives?.slice(1).map((alt) => [alt.icon, alt.external]),
    ).toEqual([
      ["ios", true],
      ["android", true],
    ]);

    const c = ctas();
    const openQr = vi.fn();
    const qr = downloadActionsProps({
      offer: mac,
      ctas: c,
      i18n,
      places,
      downloadsLabel: "x",
      campaign: {},
      openQr,
    });
    const ios = qr.alternatives![1]!;
    expect(ios.href).toBeUndefined();
    ios.onClick({} as never);
    expect(openQr).toHaveBeenCalledWith("ios");
    expect(c.leaveForStore).toHaveBeenCalledWith(
      "app_store",
      "landing-hero-platform-switch",
    );
  });

  it("offers nothing when no installer runs on the visitor's machine", () => {
    const offer = downloadOffer({
      kind: "desktop",
      os: "windows",
      arch: "ia32",
    });
    expect(
      downloadActionsProps({
        offer,
        ctas: ctas(),
        i18n,
        places,
        downloadsLabel: "x",
        campaign: {},
      }),
    ).toEqual({});
  });
});

describe("storeBadgeProps", () => {
  it("gives a phone its store badge, recorded with the surface's place", () => {
    const c = ctas();
    const props = storeBadgeProps({
      offer: downloadOffer({ kind: "mobile", os: "android" }),
      ctas: c,
      i18n,
      place: "come-hang-out",
      campaign: {},
    })!;

    expect(props.badge).toBe("google_play");
    props.onClick({} as never);
    expect(c.leaveForStore).toHaveBeenCalledWith(
      "google_play",
      "come-hang-out",
    );
  });

  it("is absent on desktop", () => {
    expect(
      storeBadgeProps({
        offer: mac,
        ctas: ctas(),
        i18n,
        place: "landing-hero",
        campaign: {},
      }),
    ).toBeUndefined();
  });
});
