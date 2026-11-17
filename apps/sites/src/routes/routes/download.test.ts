import { RouterContextProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));

import { loader as downloadLoader } from "./download";
import { loader as creatorHubLoader } from "./landings.creator-hub-download";

const LINUX_CHROME = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const MAC_CHROME = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const IPHONE = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";

const args = (path: string, userAgent = LINUX_CHROME) =>
  ({ request: new Request("https://sites.test" + path, { headers: { "user-agent": userAgent } }), params: {}, context: new RouterContextProvider() }) as never;

async function thrown(run: () => Promise<unknown>): Promise<Response> {
  try {
    await run();
  } catch (error) {
    if (error instanceof Response) return error;
    throw error;
  }
  throw new Error("the loader did not redirect");
}

// A deployment names its installer host; the node serves none.
const INSTALLERS = "https://installers.test";
beforeEach(() => vi.stubEnv("EXPLORER_INSTALLERS_URL", INSTALLERS));
afterEach(() => vi.unstubAllEnvs());

describe("/download", () => {
  it("opens a deep link at /play with its position and realm", async () => {
    const response = await thrown(() => downloadLoader(args("/download?position=10,20&realm=main&utm=x")));
    expect(response.status).toBe(302);
    expect(response.headers.get("Location")).toBe("/play/?position=10%2C20&realm=main");
  });

  it("offers Windows first and macOS second to a Linux desktop", async () => {
    const { data } = await downloadLoader(args("/download"));
    expect(data.primary).toMatchObject({ os: "windows", arch: "amd64" });
    expect(data.secondary).toMatchObject({ os: "macos", arch: "arm64" });
    expect(data.android).toBe(false);
  });

  it("offers a Mac its Apple Silicon installer first and Windows second", async () => {
    const { data } = await downloadLoader(args("/download", MAC_CHROME));
    expect(data.primary).toMatchObject({ os: "macos", arch: "arm64" });
    expect(data.secondary).toMatchObject({ os: "windows" });
  });

  it("links each installer under the configured host", async () => {
    const { data } = await downloadLoader(args("/download"));
    expect(data.primary?.url).toBe(`${INSTALLERS}/launcher-rust/Decentraland_installer.exe`);
    expect(data.secondary.url).toBe(`${INSTALLERS}/launcher-rust/Decentraland_installer.dmg`);
  });

  it("offers no installer link when the deployment configures no installer host", async () => {
    vi.stubEnv("EXPLORER_INSTALLERS_URL", "");
    const { data } = await downloadLoader(args("/download"));
    expect(data.primary?.url).toBeNull();
    expect(data.secondary.url).toBeNull();
  });

  it("offers a phone no installer", async () => {
    const { data } = await downloadLoader(args("/download", IPHONE));
    expect(data.primary).toBeNull();
  });
});

describe("/landings/creator-hub-download?step=success", () => {
  it("opens the Creator Hub steps page for the detected system", async () => {
    const response = await thrown(() => creatorHubLoader(args("/landings/creator-hub-download?step=success", MAC_CHROME)));
    expect(response.headers.get("Location")).toMatch(/^\/download\/creator-hub-success\?os=macos&arch=(arm64|amd64)$/);
  });
});

describe("/download/creator-hub", () => {
  it("offers the Creator Hub installer for the visitor's desktop system", async () => {
    const { data } = await creatorHubLoader(args("/download/creator-hub", MAC_CHROME));
    expect(data.primary).toMatchObject({ osKey: "macos" });
  });

  it("offers no installer to a system without one", async () => {
    for (const userAgent of [LINUX_CHROME, IPHONE]) {
      const { data } = await creatorHubLoader(args("/download/creator-hub", userAgent));
      expect(data.primary).toBeNull();
      expect(data.secondary).toEqual([]);
    }
  });
});
