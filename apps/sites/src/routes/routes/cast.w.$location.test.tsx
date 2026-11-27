import type { ComponentType } from "react";
import { renderToString } from "react-dom/server";
import { createStaticHandler, createStaticRouter, data, RouterContextProvider, StaticRouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/core/lib/telemetry/page-loader", () => ({
  pageLoader: async () => ({ sid: "s", assignment: { variant: "live" }, wrap: (payload: unknown) => data(payload) }),
}));
vi.mock("@/data/lib/catalyst/places/world-scenes", () => ({ loadWorldScenes: vi.fn() }));
vi.mock("@/features/social/cast-live/LiveCastWatch", () => ({ default: vi.fn(() => null) }));

import LiveCastWatch from "@/features/social/cast-live/LiveCastWatch";
import { loadWorldScenes } from "@/data/lib/catalyst/places/world-scenes";
import CastWatchPage, { loader } from "./cast.w.$location";
import CastWatchParcelPage, { loader as parcelLoader } from "./cast.w.$location.parcel.$parcel";

const WORLD = "pixelpixie.dcl.eth";
const SCENES = [
  { entityId: "scene-a", base: "0,0", title: "Main stage" },
  { entityId: "scene-b", base: "4,2", title: "Second stage" },
];
function args(location: string, signal?: AbortSignal, parcel?: string) {
  return {
    request: new Request(`https://sites.test/cast/w/${location}`, { signal }),
    params: { location, parcel },
    context: new RouterContextProvider(),
  };
}

async function renderRoute(Component: ComponentType, result: unknown) {
  const handler = createStaticHandler([{ path: "/", Component, loader: () => result }]);
  const context = await handler.query(new Request("https://sites.test/"));
  if (context instanceof Response) throw new Error(`Unexpected response: ${context.status}`);
  renderToString(<StaticRouterProvider context={context} router={createStaticRouter(handler.dataRoutes, context)} hydrate={false} />);
}

beforeEach(() => {
  vi.mocked(loadWorldScenes).mockResolvedValue(SCENES);
});
afterEach(() => {
  vi.resetAllMocks();
  vi.restoreAllMocks();
});

describe("world watcher route", () => {
  it("loads world scenes and passes them to the watcher", async () => {
    const result = await loader(args(WORLD) as never);
    expect(loadWorldScenes).toHaveBeenCalledWith(WORLD, { signal: expect.any(AbortSignal) });
    expect(result.data.scenes).toEqual(SCENES);
    await renderRoute(CastWatchPage as ComponentType, result);
    expect(vi.mocked(LiveCastWatch).mock.calls[0][0]).toEqual(expect.objectContaining({
      location: WORLD, scenes: SCENES,
    }));
  });

  it("skips the scene lookup for Genesis", async () => {
    const result = await loader(args("0,0") as never);
    expect(result.data.scenes).toBeNull();
    expect(loadWorldScenes).not.toHaveBeenCalled();
  });

  it("passes an explicit parcel without looking up scenes", async () => {
    const result = await parcelLoader(args(WORLD, undefined, "9,9") as never);
    expect(loadWorldScenes).not.toHaveBeenCalled();
    await renderRoute(CastWatchParcelPage as ComponentType, result);
    expect(vi.mocked(LiveCastWatch).mock.calls[0][0]).toEqual(expect.objectContaining({
      location: WORLD, parcel: "9,9",
    }));
    expect(vi.mocked(LiveCastWatch).mock.calls[0][0].scenes).toBeUndefined();
  });

  it("falls back to no scenes when the lookup fails", async () => {
    vi.mocked(loadWorldScenes).mockResolvedValue(null);
    expect((await loader(args(WORLD) as never)).data.scenes).toBeNull();
  });

  it("stops a slow scene lookup after three seconds and serves the page", async () => {
    const deadline = new AbortController();
    const timeout = vi.spyOn(AbortSignal, "timeout").mockReturnValue(deadline.signal);
    vi.mocked(loadWorldScenes).mockImplementation(async (_world, { signal } = {}) => {
      expect(signal).toBeDefined();
      return new Promise((_resolve, reject) => {
        signal!.addEventListener("abort", () => reject(signal!.reason), { once: true });
      });
    });
    const pending = loader(args(WORLD) as never);
    await vi.waitFor(() => expect(loadWorldScenes).toHaveBeenCalledTimes(1));
    expect(timeout).toHaveBeenCalledWith(3000);
    deadline.abort(new DOMException("Scene lookup timed out", "TimeoutError"));
    expect((await pending).data.scenes).toBeNull();
  });

  it("propagates cancellation by the client", async () => {
    const client = new AbortController();
    vi.mocked(loadWorldScenes).mockImplementation(async (_world, { signal } = {}) =>
      new Promise((_resolve, reject) => {
        signal!.addEventListener("abort", () => reject(signal!.reason), { once: true });
      }),
    );
    const pending = loader(args(WORLD, client.signal) as never);
    const rejected = expect(pending).rejects.toMatchObject({ name: "AbortError" });
    await vi.waitFor(() => expect(loadWorldScenes).toHaveBeenCalledTimes(1));
    client.abort();
    await rejected;
  });
});
