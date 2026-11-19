import { RouterContextProvider } from "react-router";
import { beforeEach, expect, it, vi } from "vitest";
import { fetchHotScenes } from "@/data/lib/create-landing/live-scenes";
import { loadLatestRelease } from "@/data/lib/download/creator-hub.server";
import { loader, sdkWorkspaceHandoff } from "./create._index";

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

async function thrown(run: () => Promise<unknown>): Promise<unknown> {
  try {
    await run();
  } catch (error) {
    return error;
  }
  throw new Error("the loader returned instead of redirecting");
}

// The link `dcl-scene-cli start` prints is `/create?projectUrl=…#token=…`. The token lives in the
// fragment, which never reaches this server, and reaches the editor route only because a
// document navigation carries a fragment across a redirect whose Location has none (RFC 9110
// §10.2.2). Handling projectUrl in a clientLoader, or answering with a Location that has its own
// fragment, would drop the token with no other failing test: this one pins the shape.
it("hands /create?projectUrl= to the SDK workspace by a fragment-free document redirect", async () => {
  const request = new Request("https://sites.test/create?projectUrl=http%3A%2F%2F127.0.0.1%3A8000%2F&creator=0xabc");
  const response = await thrown(() => loader({ request, context: new RouterContextProvider() } as never));
  expect(response).toBeInstanceOf(Response);
  const { status, headers } = response as Response;
  expect(status).toBe(302);
  const location = headers.get("location")!;
  expect(location).toBe("/create/scene-editor?source=sdk&projectUrl=http%3A%2F%2F127.0.0.1%3A8000%2F&from=home");
  expect(location).not.toContain("#");
  expect(new URL(location, "https://sites.test").searchParams.has("token")).toBe(false);
  expect(headers.get("cache-control")).toBe("private, no-store");
  expect(fetchHotScenes).not.toHaveBeenCalled();
});

it("only a projectUrl is a handoff, and a token in the query is not carried", () => {
  expect(sdkWorkspaceHandoff(new URL("https://sites.test/create"))).toBeNull();
  expect(sdkWorkspaceHandoff(new URL("https://sites.test/create?projectUrl=%20"))).toBeNull();
  const handoff = sdkWorkspaceHandoff(new URL("https://sites.test/create?projectUrl=http%3A%2F%2F127.0.0.1%3A8000&token=leaked"))!;
  expect(new URLSearchParams(handoff.split("?")[1]).get("token")).toBeNull();
  expect(new URLSearchParams(handoff.split("?")[1]).get("projectUrl")).toBe("http://127.0.0.1:8000");
});

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
