import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { loadLatestRelease, resetLatestRelease, resolveDownload } from "./creator-hub.server";

const RELEASES_PAGE = "https://github.com/decentraland/creator-hub/releases/latest";
const fetchMock = vi.fn();

function release(version: string) {
  const base = `https://github.com/decentraland/creator-hub/releases/download/${version}`;
  return {
    tag_name: version,
    assets: [
      `Decentraland-Creator-Hub-${version}-mac-arm64.dmg`,
      `Decentraland-Creator-Hub-${version}-mac-arm64.zip`,
      `Decentraland-Creator-Hub-${version}-mac-x64.dmg`,
      `Decentraland-Creator-Hub-${version}-win-x64.exe`,
      `Decentraland-Creator-Hub-${version}-win-x64.exe.blockmap`,
      "latest.yml",
    ].map((name) => ({ name, browser_download_url: `${base}/${name}` })),
  };
}

const answer = (body: unknown, status = 200) =>
  fetchMock.mockResolvedValueOnce(new Response(JSON.stringify(body), { status }));

beforeEach(() => {
  vi.stubGlobal("fetch", fetchMock);
  vi.useFakeTimers();
  resetLatestRelease();
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
  fetchMock.mockReset();
});

describe("the latest Creator Hub release", () => {
  it("offers only the three installers of the latest release", async () => {
    answer(release("0.42.0"));
    const r = await loadLatestRelease();
    expect(r.version).toBe("0.42.0");
    expect(Object.fromEntries(Object.entries(r.options).map(([k, o]) => [k, o.fileName]))).toEqual({
      "macos-arm64": "Decentraland-Creator-Hub-0.42.0-mac-arm64.dmg",
      "macos-amd64": "Decentraland-Creator-Hub-0.42.0-mac-x64.dmg",
      "windows-amd64": "Decentraland-Creator-Hub-0.42.0-win-x64.exe",
    });
  });

  it("reads GitHub once per ten minutes for every visitor", async () => {
    answer(release("0.42.0"));
    await Promise.all([loadLatestRelease(), loadLatestRelease()]);
    vi.advanceTimersByTime(9 * 60_000);
    await loadLatestRelease();
    expect(fetchMock).toHaveBeenCalledOnce();
  });

  it("shows a newer release once the cached one expires", async () => {
    answer(release("0.42.0"));
    await loadLatestRelease();
    vi.advanceTimersByTime(11 * 60_000);
    answer(release("0.43.0"));
    expect((await loadLatestRelease()).version).toBe("0.42.0");
    await vi.waitFor(async () => expect((await loadLatestRelease()).version).toBe("0.43.0"));
  });

  it("keeps the last release read when GitHub fails", async () => {
    answer(release("0.42.0"));
    await loadLatestRelease();
    vi.advanceTimersByTime(21 * 60_000);
    answer({ message: "API rate limit exceeded" }, 403);
    expect((await loadLatestRelease()).version).toBe("0.42.0");
  });

  it("links every button to the releases page when no release was ever read", async () => {
    fetchMock.mockRejectedValueOnce(new Error("network"));
    const d = resolveDownload(await loadLatestRelease(), "Mozilla/5.0 (Windows NT 10.0; Win64; x64)");
    expect(d.primary).toMatchObject({ osKey: "windows", url: RELEASES_PAGE });
    expect(d.secondary.map((o) => o.url)).toEqual([RELEASES_PAGE]);
  });

  it("does not ask again for a minute after a failed read", async () => {
    fetchMock.mockRejectedValueOnce(new Error("network"));
    await loadLatestRelease();
    vi.advanceTimersByTime(30_000);
    await loadLatestRelease();
    expect(fetchMock).toHaveBeenCalledOnce();
  });
});
