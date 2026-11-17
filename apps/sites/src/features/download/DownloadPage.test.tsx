// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { DownloadPage } from "@dcl/one-ui/apps/sites/download/DownloadPage";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$:
      () =>
      <T,>(type: T) =>
        type,
  }),
);

afterEach(cleanup);

describe("DownloadPage", () => {
  let desktopActions: ComponentProps<typeof DownloadPage>["actions"];
  let mobileStore: ComponentProps<typeof DownloadPage>["store"];

  beforeEach(() => {
    desktopActions = {
      primary: {
        label: "Download for",
        os: "windows",
        osName: "Windows",
        href: "https://explorer-artifacts.decentraland.org/launcher-rust/Decentraland_installer.exe",
      },
      alternatives: [
        {
          icon: "macos",
          label: "Download for macOS",
          href: "https://explorer-artifacts.decentraland.org/launcher-rust/Decentraland_installer.dmg",
        },
      ],
      downloads: "Total Downloads: +438K",
    };
    mobileStore = {
      badge: "google_play",
      label: "Get it on Google Play",
      href: "https://play.google.com/store/apps/details?id=org.decentraland.godotexplorer",
    };
  });

  describe("when desktop action props are supplied", () => {
    it("should render configured installer links and the download count", () => {
      render(
        <DownloadPage
          title="Download Decentraland"
          actions={desktopActions}
          store={mobileStore}
        />,
      );

      expect(
        screen.getByRole("heading", { name: "Download Decentraland" })
          .textContent,
      ).toBe("Download Decentraland");
      expect(
        screen
          .getByRole("link", { name: /Download for Windows/ })
          .getAttribute("href"),
      ).toBe(
        "https://explorer-artifacts.decentraland.org/launcher-rust/Decentraland_installer.exe",
      );
      expect(
        screen
          .getByRole("link", { name: /Download for macOS/ })
          .getAttribute("href"),
      ).toMatch(/Decentraland_installer\.dmg$/);
      expect(screen.getByText("Total Downloads: +438K").textContent).toBe(
        "Total Downloads: +438K",
      );
    });
  });

  describe("when a mobile store configuration is supplied", () => {
    it("should render the configured official store link", () => {
      render(
        <DownloadPage
          title="Download Decentraland"
          actions={desktopActions}
          store={mobileStore}
        />,
      );

      expect(
        screen
          .getByRole("link", { name: "Get it on Google Play" })
          .getAttribute("href"),
      ).toContain("play.google.com/store/apps/details");
    });
  });
});
