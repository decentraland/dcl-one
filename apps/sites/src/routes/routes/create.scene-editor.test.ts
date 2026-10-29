import { beforeEach, expect, it, onTestFinished, vi } from "vitest";
import { emptySeed, loadSceneEditorSeed } from "@/data/lib/catalyst/creator-hub/scene-editor";
import { loadAssetCatalog } from "@/data/lib/catalyst/creator-hub/asset-catalog.server";
import { fetchServerDraft } from "@/data/lib/catalyst/creator-hub/scene-drafts-client";
import { pageLoader } from "@/core/lib/telemetry/page-loader";
import { forgetSdkTokens } from "@/data/lib/fs/sdk-project";
import { claimSdkToken, clientLoader, loader } from "./create.scene-editor";

vi.mock("@/features/creator-hub/scene-editor-place-items/EditorWizard", () => ({ default: () => null }));
vi.mock("@/core/lib/telemetry/page-loader", () => ({ pageLoader: vi.fn() }));
vi.mock("@/data/lib/catalyst/creator-hub/asset-catalog.server", () => ({ loadAssetCatalog: vi.fn() }));
vi.mock("@/data/lib/catalyst/creator-hub/scene-drafts-client", () => ({ fetchServerDraft: vi.fn() }));
vi.mock("@/data/lib/catalyst/creator-hub/scene-editor", async (original) => ({
  ...await original<object>(), loadSceneEditorSeed: vi.fn(),
}));
vi.mock("@/data/lib/fs/handle-store", () => ({
  handleStore: { getMeta: async () => ({ title: "Local", updatedAt: 20 }) },
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

beforeEach(() => vi.clearAllMocks());

it("allocates a distinct URL identity for every New intent and retains it on reload", async () => {
  const input = "https://sites.test/create/scene-editor?new=1&template=empty&name=Same";
  const first = await loader({ request: new Request(input) } as never).catch(error => error) as Response;
  const second = await loader({ request: new Request(input) } as never).catch(error => error) as Response;
  expect(first.status).toBe(302);
  expect(first.headers.get("cache-control")).toBe("private, no-store");
  const redirected = new URL(first.headers.get("location")!, input);
  expect(redirected.searchParams.get("newProject")).toMatch(/^[a-f0-9-]{36}$/i);
  expect(second.headers.get("location")).not.toBe(first.headers.get("location"));
  expect(redirected.searchParams.get("template")).toBe("empty");
  expect(redirected.searchParams.get("name")).toBe("Same");
  vi.mocked(pageLoader).mockResolvedValue({ sid: "test", assignment: {}, wrap: (value: unknown) => value } as never);
  const payload = await loader({ request: new Request(redirected) } as never);
  expect(payload).toMatchObject({ seed: { scene: { title: "Same" } } });
});

it("redirects a client navigation's New intent to the document URL, not the data request", async () => {
  const input = "https://sites.test/create/scene-editor.data?new=1&template=empty&name=Same&_routes=routes%2Fcreate.scene-editor";
  const response = await loader({ request: new Request(input) } as never).catch(error => error) as Response;
  expect(response.status).toBe(302);
  const location = response.headers.get("location")!;
  expect(location).toMatch(/^\/create\/scene-editor\?/);
  const redirected = new URL(location, input);
  expect(redirected.searchParams.has("_routes")).toBe(false);
  expect(redirected.searchParams.get("newProject")).toMatch(/^[a-f0-9-]{36}$/i);
  expect(redirected.searchParams.get("template")).toBe("empty");
});

it.each(["source=local", "source=sdk&projectUrl=http%3A%2F%2Flocalhost", "draft=saved", "project=physical", "pointer=0,0", "world=example.eth"])("does not rekey an explicit project source (%s)", async query => {
  vi.mocked(pageLoader).mockResolvedValue({ sid: "test", assignment: {}, wrap: (value: unknown) => value } as never);
  const payload = await loader({ request: new Request(`https://sites.test/create/scene-editor?new=1&${query}`) } as never);
  expect(payload).not.toBeInstanceOf(Response);
});
it("starts scene loading alongside assignment without embedding the shared catalog", async () => {
  vi.stubEnv("BEVY_EDITOR_SCENE_URL", "/editor-scene/EditorScene/");
  onTestFinished(() => { vi.unstubAllEnvs(); });
  const experiment = deferred<Awaited<ReturnType<typeof pageLoader>>>();
  vi.mocked(pageLoader).mockReturnValue(experiment.promise);
  vi.mocked(loadSceneEditorSeed).mockResolvedValue(emptySeed("1,2"));
  vi.mocked(loadAssetCatalog).mockResolvedValue([]);
  const result = loader({ request: new Request("http://127.0.0.1:5158/create/scene-editor?pointer=1,2", {
    headers: { "x-forwarded-host": "sites.test", "x-forwarded-proto": "https" },
  }) } as never);
  await vi.waitFor(() => expect(loadSceneEditorSeed).toHaveBeenCalledOnce());
  expect(loadAssetCatalog).not.toHaveBeenCalled();
  experiment.resolve({ sid: "test", assignment: {}, wrap: (value: unknown) => value } as never);
  const payload = await result as unknown as { viewportSrc: string; previewSrc: string };
  expect(payload).toMatchObject({ seed: { scene: { pointer: "1,2" } } });
  expect(payload).not.toHaveProperty("catalog");
  const editor = new URL(payload.viewportSrc, "https://sites.test");
  expect(editor.searchParams.get("systemScene")).toBe("/editor-scene/EditorScene");
  expect(editor.hash).toMatch(/^#editorSession=[a-f0-9-]{36}$/i);
  const preview = new URL(payload.previewSrc, "https://sites.test");
  expect(preview.hash).toBe("");
  expect(preview.searchParams.has("systemScene")).toBe(false);
  for (const source of [payload.viewportSrc, payload.previewSrc]) {
    expect(new URL(source, "https://sites.test").searchParams.get("portables")).toBe("");
  }
});

it("without an editor scene a new project's viewport is the engine alone on its preview realm", async () => {
  vi.mocked(pageLoader).mockResolvedValue({ sid: "test", assignment: {}, wrap: (value: unknown) => value } as never);
  const payload = await loader({ request: new Request("https://sites.test/create/scene-editor?new=1&newProject=00000000-0000-4000-8000-000000000001") } as never) as unknown as { viewportSrc: string };
  const viewport = new URL(payload.viewportSrc, "https://sites.test");
  expect(viewport.pathname).toBe("/play/");
  expect(viewport.searchParams.get("realm")).toBe("/_project");
  expect(viewport.searchParams.get("hud")).toBe("0");
  expect(viewport.searchParams.has("systemScene")).toBe(false);
  expect(viewport.hash).toMatch(/^#editorSession=[a-f0-9-]{36}$/);
});

it("an editor scene /play would not run leaves the viewport the engine alone", async () => {
  vi.stubEnv("BEVY_EDITOR_SCENE_URL", "https://editor-scene.test/scene");
  onTestFinished(() => { vi.unstubAllEnvs(); });
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  onTestFinished(() => { warn.mockRestore(); });
  vi.mocked(pageLoader).mockResolvedValue({ sid: "test", assignment: {}, wrap: (value: unknown) => value } as never);
  const payload = await loader({ request: new Request("https://sites.test/create/scene-editor?new=1&newProject=00000000-0000-4000-8000-000000000001") } as never) as unknown as { viewportSrc: string };
  const viewport = new URL(payload.viewportSrc, "https://sites.test");
  expect(viewport.searchParams.has("systemScene")).toBe(false);
  expect(viewport.searchParams.get("hud")).toBe("0");
  expect(warn).toHaveBeenCalledOnce();
});

it.each([[10, "Local"], [30, "Server"]])("starts the signed draft alongside the route and retains newest-copy precedence (%s)", async (updatedAt, title) => {
  const base = deferred<unknown>();
  vi.mocked(fetchServerDraft).mockResolvedValue({
    id: "draft", version: 1, title: "Server", updatedAt,
    blob: { composite: "{}", title: "Server" },
  });
  const request = new Request("https://sites.test/create/scene-editor?draft=draft");
  const result = clientLoader({ request, serverLoader: () => base.promise } as never);
  await vi.waitFor(() => expect(fetchServerDraft).toHaveBeenCalledWith("draft", request.signal));
  base.resolve({ seed: emptySeed() });
  expect(await result).toMatchObject({ seed: { scene: { title } } });
});

it("launches a published World scene in its own realm and parcel", async () => {
  vi.mocked(pageLoader).mockResolvedValue({ sid: "test", assignment: {}, wrap: (value: unknown) => value } as never);
  vi.mocked(loadSceneEditorSeed).mockResolvedValue({ ...emptySeed("2,3"), scene: { ...emptySeed("2,3").scene, pointer: "mock.dcl.eth" } });
  vi.mocked(loadAssetCatalog).mockResolvedValue([]);
  const payload = await loader({ request: new Request("https://sites.test/create/scene-editor?world=mock.dcl.eth&pointer=2,3") } as never) as unknown as { viewportSrc: string; previewSrc: string };
  expect(loadSceneEditorSeed).toHaveBeenCalledWith(expect.objectContaining({ world: "mock.dcl.eth", pointer: "2,3" }));
  for (const source of [payload.viewportSrc, payload.previewSrc]) {
    const query = new URL(source, "https://sites.test").searchParams;
    expect(query.get("realm")).toMatch(/\/world\/mock\.dcl\.eth$/);
    expect(query.get("position")).toBe("2,3");
  }
});

it("propagates route failure while the optional draft chunk is still loading", async () => {
  const chunk = deferred<{ loadSceneEditorDraft: (_request: Request, base: Promise<unknown>) => Promise<unknown> }>();
  vi.doMock("@/data/lib/catalyst/creator-hub/scene-editor-drafts", () => chunk.promise);
  const failure = new Error("Navigation cancelled while loading the draft chunk");
  const base = Promise.reject(failure);
  // Keep the regression itself from leaking an unhandled rejection on the old
  // implementation; the assertion checks that the route rejects promptly.
  void base.catch(() => {});
  let observed: unknown;
  const result = clientLoader({
    request: new Request("https://sites.test/create/scene-editor?draft=draft"),
    serverLoader: () => base,
  } as never).catch(error => { observed = error; });
  try {
    await vi.waitFor(() => expect(observed).toBe(failure));
  } finally {
    chunk.resolve({ loadSceneEditorDraft: (_request, pending) => pending });
    vi.doUnmock("@/data/lib/catalyst/creator-hub/scene-editor-drafts");
    await result;
  }
});

it("claims the SDK run token from the fragment, takes it off the URL bar, and finds it again on a remount", () => {
  const replaceState = vi.fn();
  const browser = { location: { pathname: "/create/scene-editor", search: "?source=sdk&projectUrl=http%3A%2F%2F127.0.0.1%3A8000", hash: "#token=run-t0k3n&step=place" }, history: { state: { idx: 3 }, replaceState } };
  vi.stubGlobal("window", browser);
  try {
    expect(claimSdkToken("http://127.0.0.1:8000")).toBe("run-t0k3n");
    expect(replaceState).toHaveBeenCalledWith({ idx: 3 }, "", "/create/scene-editor?source=sdk&projectUrl=http%3A%2F%2F127.0.0.1%3A8000#step=place");
    // Exit to /create, then Back: the route remounts on a URL whose fragment is gone.
    browser.location.hash = "#step=place";
    expect(claimSdkToken("http://127.0.0.1:8000/")).toBe("run-t0k3n");
    expect(replaceState).toHaveBeenCalledOnce();
    // Another project's editor is not this run.
    expect(claimSdkToken("http://127.0.0.1:8001")).toBeNull();
    // Nothing claimed at all: no fragment, no memory.
    forgetSdkTokens();
    expect(claimSdkToken("http://127.0.0.1:8000")).toBeNull();
  } finally {
    forgetSdkTokens();
    vi.unstubAllGlobals();
  }
});
