import { RouterContextProvider } from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/data/lib/download/counts.server", () => ({
  loadDownloadCounts: vi.fn(),
}));
vi.mock("@/data/lib/catalyst/landings/home.server", () => ({
  loadHomeLive: vi.fn(),
}));
vi.mock("@/features/home/cards.server", () => ({
  whatsOnCards: () => [],
}));
vi.mock("@/features/home/content", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/features/home/content")>()),
  homeAgentContent: (content: unknown) => content,
}));

import { requestSessionContext } from "@/core/lib/telemetry/session";
import { loadHomeLive } from "@/data/lib/catalyst/landings/home.server";
import { loadDownloadCounts } from "@/data/lib/download/counts.server";
import { loader } from "./_index";

type Wrapped = { data: { sid: string }; init?: { headers?: HeadersInit } };

const request = (cookie?: string) => new Request("https://sites.example/", { headers: cookie ? { cookie } : {} });
const setCookies = (result: Wrapped) => new Headers(result.init?.headers).getSetCookie();

describe("/ loader session", () => {
  beforeEach(() => {
    vi.mocked(loadDownloadCounts).mockResolvedValue("+438K");
    vi.mocked(loadHomeLive).mockResolvedValue({
      content: {},
      cards: [],
      profiles: {},
    } as never);
  });

  it("uses the session the root middleware set and leaves the cookie to it", async () => {
    const context = new RouterContextProvider();
    context.set(requestSessionContext, { sid: "from-middleware", created: true });

    const result = (await loader({ request: request(), context } as never)) as unknown as Wrapped;

    expect(result.data.sid).toBe("from-middleware");
    expect(setCookies(result)).toEqual([]);
  });

  it("mints a session and sets its cookie when no middleware ran", async () => {
    const result = (await loader({ request: request(), context: new RouterContextProvider() } as never)) as unknown as Wrapped;

    expect(result.data.sid).toMatch(/.+/);
    expect(setCookies(result)).toHaveLength(1);
    expect(setCookies(result)[0]).toContain(`sid=${encodeURIComponent(result.data.sid)}`);
  });

  it("keeps an existing sid cookie and sets none", async () => {
    const result = (await loader({ request: request("sid=s1"), context: new RouterContextProvider() } as never)) as unknown as Wrapped;

    expect(result.data.sid).toBe("s1");
    expect(setCookies(result)).toEqual([]);
  });
});
