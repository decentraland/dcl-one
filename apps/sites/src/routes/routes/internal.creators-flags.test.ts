import { afterEach, describe, expect, it, vi } from "vitest";

import { resetFlags } from "@/data/lib/creators/featureFlags.server";

import { loader } from "./internal.creators-flags";

const fetchMock = vi.fn();
const service = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });
const ask = (url: string) => loader({ request: new Request(url) });

afterEach(() => {
  vi.unstubAllGlobals();
  fetchMock.mockReset();
  resetFlags();
});

describe("the creators flags endpoint", () => {
  it("answers an application's flags as read for the request's environment, asking as that environment's site", async () => {
    vi.stubGlobal("fetch", fetchMock.mockResolvedValue(service({ flags: { "builder-maintenance": true, "builder-campaign": "yes" } })));

    const res = await ask("https://decentraland.zone/internal/creators-flags?app=builder");

    expect(await res.json()).toEqual({ flags: { "builder-maintenance": true, "builder-campaign": false } });
    expect(res.headers.get("cache-control")).toBe("public, max-age=60");
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("https://feature-flags.decentraland.zone/builder.json");
    expect(new Headers(init.headers).get("referer")).toBe("https://decentraland.zone/");
  });

  it("reads production's flags for a production host whatever environment the address asks for", async () => {
    vi.stubGlobal("fetch", fetchMock.mockResolvedValue(service({ flags: {} })));

    await ask("https://decentraland.org/internal/creators-flags?app=dapps&env=dev");

    expect(fetchMock.mock.calls[0][0]).toBe("https://feature-flags.decentraland.org/dapps.json");
    expect(new Headers(fetchMock.mock.calls[0][1].headers).get("referer")).toBe("https://decentraland.org/");
  });

  it("follows the environment the address names on a host that is not Decentraland's", async () => {
    vi.stubGlobal("fetch", fetchMock.mockResolvedValue(service({ flags: {} })));

    await ask("https://node.example/internal/creators-flags?app=dapps&env=prod");

    expect(fetchMock.mock.calls[0][0]).toBe("https://feature-flags.decentraland.org/dapps.json");
  });

  it("reads the service once a minute for the same application and environment", async () => {
    vi.stubGlobal("fetch", fetchMock.mockResolvedValue(service({ flags: {} })));

    await ask("https://decentraland.zone/internal/creators-flags?app=builder");
    await ask("https://decentraland.zone/internal/creators-flags?app=builder");

    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("says the flags are unavailable, uncached, when the service fails, and reads again on the next request", async () => {
    vi.stubGlobal("fetch", fetchMock.mockResolvedValueOnce(service("down", 503)).mockResolvedValueOnce(service({ flags: { "builder-maintenance": true } })));

    const failed = await ask("https://decentraland.zone/internal/creators-flags?app=builder");
    expect(failed.status).toBe(503);
    expect(failed.headers.get("cache-control")).toBe("no-store");

    const next = await ask("https://decentraland.zone/internal/creators-flags?app=builder");
    expect(await next.json()).toEqual({ flags: { "builder-maintenance": true } });
  });

  it("refuses an application the creators pages do not read", async () => {
    vi.stubGlobal("fetch", fetchMock);

    const res = await ask("https://decentraland.zone/internal/creators-flags?app=explorer");

    expect(res.status).toBe(400);
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
