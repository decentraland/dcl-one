// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, expect, it, vi } from "vitest";

import DownloadPage from "./DownloadPage";
import sitesLogo from "@dcl/one-ui/apps/sites/assets/dcl-logo.svg?url";

vi.hoisted(() => Object.assign(window, { __vite_plugin_react_preamble_installed__: true }));
vi.mock("../components/chrome/SitesLayout", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("@dcl/one-ui/components/AppChrome/chrome-auth", () => ({ useChromeAuth: () => ({ signedIn: false, account: null, name: null, onSignIn: () => {} }) }));

afterEach(cleanup);

it("links home through the decorative 48px Decentraland logo", () => {
  render(<DownloadPage primary={WINDOWS} secondary={MACOS} android={false} downloads={null} />);
  const home = screen.getAllByRole("link", { name: "Decentraland" }).find((a) => a.getAttribute("href") === "/");
  const logo = home!.querySelector("img");
  expect(logo!.getAttribute("alt")).toBe("");
  expect([logo!.getAttribute("width"), logo!.getAttribute("height")]).toEqual(["48", "48"]);
  expect(logo!.getAttribute("src")).toBe(sitesLogo);
});

const WINDOWS = { os: "windows", arch: "amd64", url: "https://installers.test/Decentraland_installer.exe" } as const;
const MACOS = { os: "macos", arch: "arm64", url: "https://installers.test/Decentraland_installer.dmg" } as const;

it("names the page with level 1 headings only, one for each layout", () => {
  render(<DownloadPage primary={WINDOWS} secondary={MACOS} android={false} downloads="+438K" />);
  const headings = screen.getAllByRole("heading");
  expect(headings.map((h) => h.tagName)).toEqual(["H1", "H1"]);
  expect(new Set(headings.map((h) => h.textContent)).size).toBe(1);
});

it("links each installer button to the page that starts that installer, with no click handler in the way", () => {
  render(<DownloadPage primary={WINDOWS} secondary={MACOS} android={false} downloads={null} />);
  expect(screen.getByRole("link", { name: "Download for Windows" }).getAttribute("href")).toBe("/download_success?os=Windows&arch=amd64");
  expect(screen.getByRole("link", { name: "macOS" }).getAttribute("href")).toBe("/download_success?os=macOS&arch=arm64");
});
