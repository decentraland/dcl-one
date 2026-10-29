import { afterEach, expect, test, vi } from "vitest";
import { createProjectPlayStateWriter } from "./project-cache";

afterEach(() => vi.unstubAllGlobals());

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

test("drains a stale play-state true before the reset false write", async () => {
  const first = deferred();
  const started = deferred();
  const writes: string[] = [];
  const setPlayState = createProjectPlayStateWriter(async (playing) => {
    if (!playing) {
      writes.push("false");
      return;
    }
    writes.push("true:start");
    started.resolve();
    await first.promise;
    writes.push("true:end");
  }, "play-state-test");

  const playing = setPlayState(true);
  const reset = setPlayState(false);
  await started.promise;
  expect(writes).toEqual(["true:start"]);
  first.resolve();
  await playing;
  await reset;
  expect(writes).toEqual(["true:start", "true:end", "false"]);
});

test("content URLs use the captured project namespace without global server discovery", async () => {
  const { projectContentBase } = await import("./project-cache");
  const { projectScopeForId } = await import("./project-cache-name");
  const scope = projectScopeForId("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa");
  const request = vi.fn();
  vi.stubGlobal("fetch", request);
  expect(await projectContentBase(scope)).toBe(scope.contentBase);
  expect(request).not.toHaveBeenCalled();
});

test("queued playback writes stay bound to their captured project", async () => {
  const { setProjectPlayState } = await import("./project-cache");
  const { projectScopeForId } = await import("./project-cache-name");
  const a = projectScopeForId("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa");
  const b = projectScopeForId("bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb");
  const values = new Map<string, Map<string, Response>>();
  vi.stubGlobal("caches", { open: async (name: string) => {
    if (!values.has(name)) values.set(name, new Map());
    return { put: async (url: string, response: Response) => { values.get(name)!.set(url, response); } };
  } });
  await Promise.all([setProjectPlayState(true, false, a), setProjectPlayState(false, false, b)]);
  expect(await values.get(a.cacheName)!.get(`${a.contentBase}/contents/one-play-state`)!.json()).toEqual({ playing: true });
  expect(await values.get(b.cacheName)!.get(`${b.contentBase}/contents/one-play-state`)!.json()).toEqual({ playing: false });
});

test("reopened catalog mappings load only the selected project's saved asset map", async () => {
  const { placedProjectContents } = await import("./project-cache");
  const { projectScopeForId } = await import("./project-cache-name");
  const a = projectScopeForId("cccccccc-cccc-4ccc-cccc-cccccccccccc");
  const b = projectScopeForId("dddddddd-dddd-4ddd-dddd-dddddddddddd");
  sessionStorage.setItem(`ch:project-contents:${a.id}`, JSON.stringify({ "assets/mine.glb": "my-cid" }));
  sessionStorage.setItem(`ch:project-contents:${b.id}`, JSON.stringify({ "assets/theirs.glb": "their-cid" }));
  expect(placedProjectContents(a)).toEqual({ "assets/mine.glb": "my-cid" });
  expect(placedProjectContents(b)).toEqual({ "assets/theirs.glb": "their-cid" });
});
