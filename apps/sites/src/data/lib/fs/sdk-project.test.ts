import { afterEach, expect, it, vi } from "vitest";
import { SdkProjectConnection, forgetSdkTokens, recallSdkToken, rememberSdkToken, sdkProjectUrl, sdkTokenFromHash } from "./sdk-project";

afterEach(() => { vi.unstubAllGlobals(); forgetSdkTokens(); });

it("calls the browser fetch without binding it to the project instance", async () => {
  const fetcher = vi.fn(function (this: unknown) {
    expect(this).toBeUndefined();
    return Promise.resolve(Response.json({ files: [] }));
  });
  vi.stubGlobal("fetch", fetcher);
  await expect(new SdkProjectConnection("http://localhost:8000").list()).resolves.toEqual([]);
  expect(fetcher).toHaveBeenCalledOnce();
});

it("accepts local SDK and HTTPS proxy prefixes, rejects unsafe launch URLs", () => {
  expect(sdkProjectUrl("http://127.0.0.1:8000/")).toBe("http://127.0.0.1:8000");
  expect(sdkProjectUrl("https://example.test/project/")).toBe("https://example.test/project");
  for (const url of ["javascript:alert(1)", "http://remote.test", "https://user:pass@example.test", "https://example.test/?token=secret"]) {
    expect(() => sdkProjectUrl(url)).toThrow();
  }
});

it("writes with the revision originally read, preserves conflicts, and supports new files", async () => {
  const fetcher = vi.fn<typeof fetch>();
  fetcher.mockResolvedValueOnce(Response.json({ path: "src/index.ts", content: "old", revision: "r1" }));
  fetcher.mockResolvedValueOnce(new Response("conflict", { status: 409 }));
  fetcher.mockResolvedValueOnce(Response.json({ path: "src/new.ts", content: "new", revision: "r2" }));
  const project = new SdkProjectConnection("https://sdk.test/prefix", fetcher);
  await project.read("src/index.ts");
  await expect(project.write("src/index.ts", "mine")).rejects.toThrow("changed outside");
  expect(JSON.parse(fetcher.mock.calls[1]![1]!.body as string)).toEqual({ content: "mine", revision: "r1" });
  await project.write("src/new.ts", "new");
  expect(JSON.parse(fetcher.mock.calls[2]![1]!.body as string).revision).toBeNull();
  expect(fetcher.mock.calls[0]![0]).toBe("https://sdk.test/prefix/api/project/file?path=src%2Findex.ts");
});

function streamedResponse(text: string): Response {
  const bytes = new TextEncoder().encode(text);
  return new Response(new ReadableStream({ start(controller) {
    for (const byte of bytes) controller.enqueue(new Uint8Array([byte]));
    controller.close();
  } }), { headers: { "content-type": "application/x-ndjson" } });
}

it("serializes overlapping autosaves within one file session without masking external conflicts", async () => {
  let content = "old", revision = "r1";
  let release!: () => void, started!: () => void;
  const released = new Promise<void>(resolve => { release = resolve; });
  const firstStarted = new Promise<void>(resolve => { started = resolve; });
  const edits: { content: string; revision: string }[] = [];
  const fetcher = vi.fn<typeof fetch>(async (_url, init) => {
    if (init?.method !== "PUT") return Response.json({ content, revision });
    const edit = JSON.parse(init.body as string);
    edits.push(edit);
    if (edit.content === "first") { started(); await released; }
    if (edit.revision !== revision) return new Response("conflict", { status: 409 });
    content = edit.content; revision += "next";
    return Response.json({ content, revision });
  });
  const project = new SdkProjectConnection("https://sdk.test", fetcher);
  await project.read("src/ui.tsx");
  const first = project.write("src/ui.tsx", "first");
  await firstStarted;
  const second = project.write("src/ui.tsx", "second");
  await project.readOnly("src/ui.tsx");
  expect(edits).toHaveLength(1);
  release();
  await Promise.all([first, second]);
  expect(edits).toEqual([{ content: "first", revision: "r1" }, { content: "second", revision: "r1next" }]);
  expect(content).toBe("second");
  content = "external"; revision = "external-r3";
  await expect(project.write("src/ui.tsx", "stale")).rejects.toThrow("changed outside");
  expect(content).toBe("external");
  expect(await project.read("src/ui.tsx")).toBe("external");
  await project.write("src/ui.tsx", "refreshed");
  expect(content).toBe("refreshed");
});

it("keeps scanner and other editor reads from advancing a consumer's save revision", async () => {
  let content = "old", revision = "r1";
  const fetcher = vi.fn<typeof fetch>(async (_url, init) => {
    if (init?.method !== "PUT") return Response.json({ path: "src/ui.tsx", content, revision });
    const edit = JSON.parse(init.body as string);
    if (edit.revision !== revision) return new Response("conflict", { status: 409 });
    content = edit.content; revision += "next";
    return Response.json({ path: "src/ui.tsx", content, revision });
  });
  const shared = new SdkProjectConnection("https://sdk.test", fetcher);
  const monaco = shared.createFileSession(), designer = shared.createFileSession();
  expect(await monaco.read("src/ui.tsx")).toBe("old");
  expect(await designer.read("src/ui.tsx")).toBe("old");
  content = "external edit"; revision = "r2";
  expect(await shared.readOnly("src/ui.tsx")).toBe("external edit");
  expect(await monaco.readOnly("src/ui.tsx")).toBe("external edit");
  await shared.read("src/ui.tsx");
  await expect(monaco.write("src/ui.tsx", "stale Monaco edit")).rejects.toThrow("changed outside");
  await expect(designer.write("src/ui.tsx", "stale designer edit")).rejects.toThrow("changed outside");
  expect(content).toBe("external edit");
  await designer.read("src/ui.tsx");
  await designer.write("src/ui.tsx", "refreshed designer edit");
  await expect(monaco.write("src/ui.tsx", "still stale")).rejects.toThrow("changed outside");
  expect(content).toBe("refreshed designer edit");
});

it("decodes assistant events across arbitrary UTF-8 boundaries and a final unterminated line", async () => {
  const events = [
    { type: "started", turnId: "turn-1", provider: "codex" },
    { type: "text", text: "A\u00f1adir un \u00e1rbol \u{1f333}" },
    { type: "done", exitCode: 0, cancelled: false },
  ];
  const request = vi.fn<typeof fetch>().mockResolvedValue(streamedResponse(events.map((event) => JSON.stringify(event)).join("\n")));
  const project = new SdkProjectConnection("https://sdk.test", request);
  const onEvent = vi.fn();
  const signal = new AbortController().signal;
  await project.assistant.turn({ provider: "codex", prompt: "Add a tree", conversationId: "conversation-1" }, onEvent, signal);
  expect(onEvent.mock.calls.map(([event]) => event)).toEqual(events);
  expect(request.mock.calls[0]![1]).toMatchObject({ credentials: "omit", signal });
  expect(JSON.parse(request.mock.calls[0]![1]!.body as string).conversationId).toBe("conversation-1");
});

it("reports a truncated stream and preserves the SDK busy error", async () => {
  const request = vi.fn<typeof fetch>()
    .mockResolvedValueOnce(streamedResponse('{"type":"text","text":"Working"}\n'))
    .mockResolvedValueOnce(new Response("Another assistant turn is running", { status: 409 }));
  const project = new SdkProjectConnection("https://sdk.test", request);
  const input = { provider: "codex", prompt: "Add a tree" };
  await expect(project.assistant.turn(input, vi.fn(), new AbortController().signal)).rejects.toThrow("disconnected before completing");
  await expect(project.assistant.turn(input, vi.fn(), new AbortController().signal)).rejects.toThrow("Another assistant turn is running");
});

it("cancels the specific assistant turn through the existing project connection", async () => {
  const request = vi.fn<typeof fetch>().mockResolvedValue(Response.json({ cancelled: true }, { status: 202 }));
  const project = new SdkProjectConnection("https://sdk.test/project", request);
  await project.assistant.cancel("turn/1");
  expect(request.mock.calls[0]![0]).toBe("https://sdk.test/project/api/project/assistant/turn/turn%2F1");
  expect(request.mock.calls[0]![1]!.method).toBe("DELETE");
});

it("requires a real device acknowledgment and preserves command errors", async () => {
  const request = vi.fn<typeof fetch>()
    .mockResolvedValueOnce(Response.json({ ok: true, data: {} }))
    .mockResolvedValueOnce(Response.json({ ok: false, data: { error: "timeout" } }));
  const project = new SdkProjectConnection("https://sdk.test", request);
  await project.debug.command(7, "pause");
  expect(JSON.parse(request.mock.calls[0]![1]!.body as string)).toEqual({ sessionId: 7, cmd: "pause" });
  await expect(project.debug.command(7, "resume")).rejects.toThrow("timeout");
});

it("accepts SDK app launch links and rejects executable links in a device descriptor", async () => {
  const descriptor = { nativeUrl: "decentraland://realm=http%3A%2F%2Flocalhost%3A8000", multiInstanceUrl: "decentraland://realm=http%3A%2F%2Flocalhost%3A8000&multi-instance=true", mobileUrl: null, mobileQr: null };
  const request = vi.fn<typeof fetch>().mockResolvedValueOnce(Response.json(descriptor))
    .mockResolvedValueOnce(Response.json({ ...descriptor, nativeUrl: "javascript:alert(1)" }));
  const project = new SdkProjectConnection("https://sdk.test", request);
  expect(await project.debug.descriptor()).toEqual(descriptor);
  await expect(project.debug.descriptor()).rejects.toThrow("unsupported device preview link");
});

it("streams device sessions and logs with the shared NDJSON decoder and reports disconnection", async () => {
  const events = [{ type: "sessions", sessions: [{ id: 7, deviceName: "Phone" }] },
    { type: "entries", sessionId: 7, seq: 1, entries: [{ message: "Loaded \u{1f333}" }] }];
  const request = vi.fn<typeof fetch>().mockResolvedValue(streamedResponse(events.map((event) => JSON.stringify(event)).join("\n")));
  const project = new SdkProjectConnection("https://sdk.test", request);
  const onEvent = vi.fn();
  await expect(project.debug.events(onEvent, new AbortController().signal)).rejects.toThrow("disconnected");
  expect(onEvent.mock.calls.map(([event]) => event)).toEqual(events);
});

it("preserves SDK asset revisions for conditional creation and removal", async () => {
  const fetcher = vi.fn<typeof fetch>();
  fetcher.mockResolvedValueOnce(new Response(null, { headers: { etag: '"reviewed"' } }));
  fetcher.mockResolvedValueOnce(new Response(null, { status: 204 }));
  fetcher.mockResolvedValueOnce(new Response(null, { status: 204 }));
  fetcher.mockResolvedValueOnce(new Response("file changed", { status: 412 }));
  const project = new SdkProjectConnection("https://sdk.test", fetcher);
  const revision = await project.assets.revision("assets/tree.glb");
  expect(fetcher.mock.calls[0]![1]!.method).toBe("HEAD");
  await project.assets.write("assets/new.glb", new ArrayBuffer(4));
  expect(fetcher.mock.calls[1]![1]!.headers).toMatchObject({ "if-none-match": "*" });
  await project.assets.remove("assets/tree.glb", revision);
  expect(fetcher.mock.calls[2]![1]!.headers).toEqual({ "if-match": '"reviewed"' });
  await expect(project.assets.remove("assets/tree.glb", revision)).rejects.toThrow(/Refresh the asset list/);
});

it("removes UI source using the last read revision and exposes only same-server designer links", async () => {
  const fetcher = vi.fn<typeof fetch>()
    .mockResolvedValueOnce(Response.json({ version: 1, name: "Scene", scene: {}, capabilities: { files: true }, links: { uiDesigner: "/inspector/", uiDesignerRuntime: "/inspector-runtime.js" } }))
    .mockResolvedValueOnce(Response.json({ content: "export const Hud = () => null", revision: "source-r1" }))
    .mockResolvedValueOnce(Response.json({ deleted: true }));
  const project = new SdkProjectConnection("https://sdk.test", fetcher);
  await project.connect();
  expect(project.uiDesigner).toEqual({ url: "https://sdk.test/inspector/", runtimeUrl: "https://sdk.test/inspector-runtime.js" });
  await expect(project.remove("src/ui/Hud.tsx")).rejects.toThrow("Read the source");
  await project.read("src/ui/Hud.tsx");
  await project.remove("src/ui/Hud.tsx");
  expect(fetcher.mock.calls[2]![1]).toMatchObject({ method: "DELETE", headers: { "if-match": "source-r1" } });
});


it("uses SDK-owned conversation history under the project URL prefix", async () => {
  const request = vi.fn<typeof fetch>().mockImplementation(async () => new Response(JSON.stringify({ conversations: [] })));
  const project = new SdkProjectConnection("https://sdk.test/project", request);
  await project.assistant.conversations();
  await project.assistant.conversation("chat/1");
  await project.assistant.deleteConversation("chat/1");
  expect(request.mock.calls.map(call => call[0])).toEqual([
    "https://sdk.test/project/api/project/assistant/conversations",
    "https://sdk.test/project/api/project/assistant/conversations/chat%2F1",
    "https://sdk.test/project/api/project/assistant/conversations/chat%2F1",
  ]);
  expect(request.mock.calls[2]![1]?.method).toBe("DELETE");
});

it("reads the run token from the page fragment and leaves the rest of the hash alone", () => {
  expect(sdkTokenFromHash("")).toEqual({ token: null, rest: "" });
  expect(sdkTokenFromHash("#step=place")).toEqual({ token: null, rest: "step=place" });
  expect(sdkTokenFromHash("#token=abc-DEF_1&step=place")).toEqual({ token: "abc-DEF_1", rest: "step=place" });
  expect(sdkTokenFromHash("token=abc")).toEqual({ token: "abc", rest: "" });
  expect(sdkTokenFromHash("#token=")).toEqual({ token: null, rest: "token=" });
});

it("sends the run token as a bearer on every project request, on file sessions, and on nothing without one", async () => {
  const bare = vi.fn<typeof fetch>().mockResolvedValue(Response.json({ files: [] }));
  await new SdkProjectConnection("https://sdk.test", bare).list();
  expect(new Headers(bare.mock.calls[0]![1]!.headers).get("authorization")).toBeNull();
  const fetcher = vi.fn<typeof fetch>(async (url, init) => {
    if (init?.method === "HEAD" || init?.method === "DELETE") return new Response(null, { status: 204, headers: { etag: '"r"' } });
    if (String(url).endsWith("/assistant/turn")) return streamedResponse('{"type":"done","exitCode":0,"cancelled":false}\n');
    return Response.json({ path: "src/index.ts", content: "x", revision: "r1", files: [], ok: true, data: {} });
  });
  const project = new SdkProjectConnection("https://sdk.test", fetcher, "run-t0k3n");
  const session = project.createFileSession();
  await session.read("src/index.ts");
  await session.write("src/index.ts", "y");
  await project.assets.revision("assets/a.glb");
  await project.assets.write("assets/a.glb", new ArrayBuffer(2));
  await project.assets.remove("assets/a.glb", '"r"');
  await project.assistant.turn({ provider: "codex", prompt: "hi" }, vi.fn(), new AbortController().signal);
  await project.debug.command(1, "pause");
  expect(fetcher.mock.calls.length).toBe(7);
  for (const [, init] of fetcher.mock.calls) expect(new Headers(init!.headers).get("authorization")).toBe("Bearer run-t0k3n");
  expect(new Headers(fetcher.mock.calls[1]![1]!.headers).get("content-type")).toBe("application/json");
  expect(new Headers(fetcher.mock.calls[4]![1]!.headers).get("if-match")).toBe('"r"');
  fetcher.mockResolvedValueOnce(new Response("token required", { status: 401 }));
  await expect(project.list()).rejects.toThrow(/did not accept this page's token/);
});

it("puts the token on the data-layer upgrade URL only, and only for the project's own server", async () => {
  const links = { files: "/api/project/files", file: "/api/project/file", reload: "/", preview: "/", settings: "/", publish: "/", storage: "/", inspector: null, dataLayer: "/data-layer", mcp: null };
  const descriptor = { version: 1, name: "Scene", scene: {}, capabilities: { files: true, write: true, watch: true, dataLayer: true, mcp: false }, links };
  const fetcher = vi.fn<typeof fetch>()
    .mockResolvedValueOnce(Response.json(descriptor))
    .mockResolvedValueOnce(Response.json({ ...descriptor, links: { ...links, dataLayer: "ws://127.0.0.1:8000/data-layer" } }))
    .mockResolvedValueOnce(Response.json({ ...descriptor, links: { ...links, dataLayer: null } }))
    .mockResolvedValueOnce(Response.json({ ...descriptor, links: { ...links, dataLayer: "ws://evil.test/data-layer" } }));
  const project = new SdkProjectConnection("http://127.0.0.1:8000", fetcher, "run-t0k3n");
  await project.connect();
  expect(project.dataLayerUrl).toBe("http://127.0.0.1:8000/data-layer?token=run-t0k3n");
  await project.connect();
  expect(project.dataLayerUrl).toBe("ws://127.0.0.1:8000/data-layer?token=run-t0k3n");
  await project.connect();
  expect(project.dataLayerUrl).toBeNull();
  await project.connect();
  expect(() => project.dataLayerUrl).toThrow("another server");
  const anonymous = new SdkProjectConnection("http://127.0.0.1:8000", vi.fn<typeof fetch>().mockResolvedValue(Response.json(descriptor)));
  await anonymous.connect();
  expect(anonymous.dataLayerUrl).toBe("http://127.0.0.1:8000/data-layer");
});

it("keeps a claimed token in memory by project URL, whatever the spelling of the URL", () => {
  expect(recallSdkToken("http://127.0.0.1:8000")).toBeNull();
  rememberSdkToken("http://127.0.0.1:8000/", "run-t0k3n");
  expect(recallSdkToken("http://127.0.0.1:8000")).toBe("run-t0k3n");
  expect(recallSdkToken("http://127.0.0.1:8000/")).toBe("run-t0k3n");
  expect(recallSdkToken("http://127.0.0.1:8001")).toBeNull();
  rememberSdkToken("javascript:alert(1)", "ignored");
  expect(recallSdkToken("javascript:alert(1)")).toBeNull();
  forgetSdkTokens();
  expect(recallSdkToken("http://127.0.0.1:8000")).toBeNull();
});

it("points every 401 at the SDK link, whichever route refused the token", async () => {
  const refused = vi.fn<typeof fetch>().mockImplementation(async () => new Response("token required", { status: 401 }));
  const project = new SdkProjectConnection("https://sdk.test", refused, "stale");
  const hint = /did not accept this page's token/;
  await expect(project.assets.remove("assets/tree.glb", '"r"')).rejects.toThrow(hint);
  await expect(project.assets.write("assets/tree.glb", new ArrayBuffer(2))).rejects.toThrow(hint);
  await expect(project.assets.read("assets/tree.glb")).rejects.toThrow(hint);
  await expect(project.assets.revision("assets/tree.glb")).rejects.toThrow(hint);
  await expect(project.assistant.turn({ provider: "codex", prompt: "hi" }, vi.fn(), new AbortController().signal)).rejects.toThrow(hint);
  await expect(project.debug.events(vi.fn(), new AbortController().signal)).rejects.toThrow(hint);
  await expect(project.debug.command(1, "pause")).rejects.toThrow(hint);
  await expect(project.list()).rejects.toThrow(hint);
  expect(refused).toHaveBeenCalledTimes(8);
  // Other statuses keep their route-specific reading.
  const missing = vi.fn<typeof fetch>().mockImplementation(async () => new Response("gone", { status: 412 }));
  const other = new SdkProjectConnection("https://sdk.test", missing, "run-t0k3n");
  await expect(other.assets.remove("assets/tree.glb", '"r"')).rejects.toThrow(/Refresh the asset list/);
  await expect(other.assets.write("assets/tree.glb", new ArrayBuffer(2))).rejects.toThrow(/Could not save assets\/tree.glb \(412\)\. gone/);
  await expect(other.assistant.turn({ provider: "codex", prompt: "hi" }, vi.fn(), new AbortController().signal)).rejects.toThrow(/Assistant request failed \(412\): gone/);
});
