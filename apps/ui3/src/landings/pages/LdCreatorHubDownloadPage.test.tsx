import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vitest";

import LdCreatorHubDownloadPage from "./LdCreatorHubDownloadPage";

type Option = { os: string; osKey: "windows" | "macos"; url: string; arch: string; archLabel?: string };

const MAC: Option = { os: "macOS", osKey: "macos", url: "https://example.com/hub.dmg", arch: "arm64" };
const WIN: Option = { os: "Windows", osKey: "windows", url: "https://example.com/hub.exe", arch: "amd64" };

function glyphFills(html: string): string[] {
  return [...html.matchAll(/src="data:image\/svg\+xml;utf8,([^"]+)"/g)].map((m) => {
    const svg = decodeURIComponent((m[1] ?? "").replaceAll("&amp;", "&"));
    return /fill="([^"]*)"/.exec(svg)?.[1] ?? "";
  });
}

describe("when the Creator Hub download page offers a primary and an alternative installer", () => {
  it("should draw both system glyphs in white", () => {
    const html = renderToString(<LdCreatorHubDownloadPage version="1.0.0" primary={MAC} secondary={[WIN]} />);
    const fills = glyphFills(html);
    expect(fills).toHaveLength(2);
    expect(fills).toEqual(["#fff", "#fff"]);
  });
});

describe("when the visitor's system has no installer", () => {
  it("should draw no download glyphs", () => {
    const html = renderToString(<LdCreatorHubDownloadPage version="1.0.0" primary={null} secondary={[WIN]} />);
    expect(glyphFills(html)).toHaveLength(0);
  });
});

describe("when a system has two builds", () => {
  it("should name each Mac build by its chip in the alternatives", () => {
    const silicon: Option = { ...MAC, archLabel: "Apple Silicon" };
    const intel: Option = { ...MAC, url: "https://example.com/hub-x64.dmg", arch: "amd64", archLabel: "Intel" };
    const html = renderToString(<LdCreatorHubDownloadPage version="1.0.0" primary={silicon} secondary={[intel, WIN]} />);
    expect(html).toContain('aria-label="Download for macOS (Intel)"');
    expect(html).toContain('aria-label="Download for Windows"');
    expect(html).toContain(">Intel</span>");
  });
});
