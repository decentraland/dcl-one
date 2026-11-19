import { RouterContextProvider } from "react-router";
import { beforeEach, expect, it, vi } from "vitest";
import { fetchHotScenes } from "@/data/lib/create-landing/live-scenes";
import { loadLatestRelease } from "@/data/lib/download/creator-hub.server";
import { loader } from "./create._index";

vi.mock("@/data/lib/create-landing/live-scenes", async (actual) => ({
  ...(await actual<typeof import("@/data/lib/create-landing/live-scenes")>()),
  fetchHotScenes: vi.fn(),
}));
vi.mock("@/core/lib/telemetry/page-loader", () => ({
  pageLoader: vi.fn(),
  sidLoader: () => ({ sid: "s1", wrap: (payload: unknown) => ({ data: payload }) }),
}));
vi.mock("@/data/lib/create-landing/blog-posts.server", () => ({ latestPosts: vi.fn(async () => []) }));
vi.mock("@/data/lib/download/creator-hub.server", async (actual) => ({
  ...(await actual<typeof import("@/data/lib/download/creator-hub.server")>()),
  loadLatestRelease: vi.fn(),
}));

beforeEach(() => vi.clearAllMocks());

const WINDOWS = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";
const BASE = "https://github.com/decentraland/creator-hub/releases/download/1.2.3";
const build = (os: string, osKey: string, arch: string, archLabel: string, fileName: string) => ({
  os,
  osKey,
  arch,
  archLabel,
  fileName,
  url: `${BASE}/${fileName}`,
});
const RELEASE = {
  version: "1.2.3",
  options: {
    "macos-arm64": build("macOS", "macos", "arm64", "Apple Silicon", "hub-mac-arm64.dmg"),
    "macos-amd64": build("macOS", "macos", "amd64", "Intel", "hub-mac-x64.dmg"),
    "windows-amd64": build("Windows", "windows", "amd64", "x64", "hub-win-x64.exe"),
  },
} as Awaited<ReturnType<typeof loadLatestRelease>>;

async function landingData(userAgent: string) {
  vi.mocked(fetchHotScenes).mockResolvedValue([]);
  const request = new Request("https://sites.test/create", { headers: { "user-agent": userAgent } });
  const result = (await loader({ request, context: new RouterContextProvider() } as never)) as unknown as {
    data: { downloads: { primary: { osKey: string }; others: unknown[] } | null; sid: string };
  };
  return result.data;
}

it("hands the hero the installer for the visitor's system and the other builds", async () => {
  vi.mocked(loadLatestRelease).mockResolvedValue(RELEASE);
  const { downloads, sid } = await landingData(WINDOWS);
  expect(downloads?.primary.osKey).toBe("windows");
  expect(downloads?.others).toHaveLength(2);
  expect(sid).toEqual(expect.any(String));
});

it("does not hold the landing for a release that is slow to read", async () => {
  vi.mocked(loadLatestRelease).mockReturnValue(new Promise(() => {}));
  const { downloads } = await landingData(WINDOWS);
  expect(downloads).toBeNull();
});
