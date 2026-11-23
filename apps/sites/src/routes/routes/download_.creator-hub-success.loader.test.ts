import { RouterContextProvider } from "react-router";
import { describe, expect, it, vi } from "vitest";

import { loader } from "./download_.creator-hub-success";

vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/data/lib/download/creator-hub.server", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/data/lib/download/creator-hub.server")>();
  const BASE = "https://github.com/decentraland/creator-hub/releases/download/1.2.3";
  const opt = (os: "macOS" | "Windows", osKey: "macos" | "windows", arch: "arm64" | "amd64", fileName: string) => ({
    os,
    osKey,
    arch,
    archLabel: "",
    fileName,
    url: `${BASE}/${fileName}`,
  });
  return {
    ...actual,
    loadLatestRelease: async () => ({
      version: "1.2.3",
      options: {
        "macos-arm64": opt("macOS", "macos", "arm64", "hub-mac-arm64.dmg"),
        "macos-amd64": opt("macOS", "macos", "amd64", "hub-mac-x64.dmg"),
        "windows-amd64": opt("Windows", "windows", "amd64", "hub-win-x64.exe"),
      },
    }),
  };
});

const ANON = "0b7f4c2e-1d3a-4e5f-9a8b-7c6d5e4f3a2b";
type Data = { arch: string; exact: string | null; anonUserId: string | null };

describe("the steps page's reading of its query (legacy clientOS, clientArch, osLink)", () => {
  const load = async (query: string) =>
    ((await loader({ request: new Request(`https://sites.test/download/creator-hub-success${query}`), params: {}, context: new RouterContextProvider() } as never)) as unknown as { data: Data }).data;

  it("reports the exact build the query names, and the URL's anonymous id as given", async () => {
    expect(await load(`?os=Windows&arch=amd64&anon_user_id=${ANON}`)).toMatchObject({
      arch: "amd64",
      exact: expect.stringMatching(/hub-win-x64\.exe$/),
      anonUserId: ANON,
    });
  });

  it("defaults the arch per system and drops one outside the allowlist", async () => {
    expect(await load("?os=macOS&arch=x86")).toMatchObject({ arch: "arm64", exact: expect.stringMatching(/hub-mac-arm64\.dmg$/) });
  });

  it("reports no build the release does not hold (Windows on Arm)", async () => {
    expect(await load("?os=windows&arch=arm64")).toMatchObject({ arch: "arm64", exact: null });
  });
});
