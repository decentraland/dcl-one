import { act, cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import DeWorkspace from "./DeWorkspace";
import { SCENE_BRIDGE_VERSION } from "../bus-protocol";
import { editViewRevealed, OVERHEAD_CAMERA_POSE, settleEngineWorld, useEditorClock } from "../../test/editorViewport";
import { sceneCheckpointStore, type SceneCheckpoint } from "../scene-recovery";
import { RECOVERY_DELAY_MS, useSceneRecovery } from "./useSceneRecovery";
import { ProjectUploadUnavailableError } from "../project-cache-name";
import type { CaptureAuthoredScene } from "../types";

type Envelope = { to: string; msg: Record<string, unknown> };
class Channel {
  static instances: Channel[] = [];
  static sent: Envelope[] = [];
  static runtime = 1;
  static tick = 3;
  static broken = false;
  onmessage: ((event: { data: Envelope }) => void) | null = null;
  closed = false;
  constructor(readonly name: string) { Channel.instances.push(this); }
  postMessage(value: Envelope) {
    Channel.sent.push(value);
    for (const peer of Channel.instances) if (peer !== this && !peer.closed && peer.name === this.name) peer.onmessage?.({ data: value });
    if (value.msg.type === "rpc" && value.msg.method === "sceneStats") queueMicrotask(() => {
      if (!this.closed) this.onmessage?.({ data: { to: "page", msg: { type: "rpc-reply", id: value.msg.id, ok: true, result: `hash: scene\ninstance: ${Channel.runtime}\ntick: ${Channel.tick}\nstatus: blocked\nbroken: ${Channel.broken}` } } });
    });
  }
  close() { this.closed = true; }
}
const saved = '{"components":[]}';
const edited = '{"components":[{"name":"core::Name","data":{"42":{"json":{"value":"Recovered crate"}}}}]}';
const viewportSrc = "/_play?editorSession=00000000-0000-4000-8000-000000000001&realm=project";
const ready = { type: "scene-ready", bridge: SCENE_BRIDGE_VERSION, scene: { hash: "scene", title: "Project", parcels: [], isPortable: false, isBroken: false, isBlocked: true, isSuper: false, sdkVersion: "7" }, frozen: true, tool: "translate", orientGlobal: false, pivotEach: false, selected: [], active: null };
const rpcs = (method: string) => Channel.sent.filter(({ msg }) => msg.type === "rpc" && msg.method === method).map(({ msg }) => msg);
const records = new Map<string, SceneCheckpoint>();
function deliver(msg: Record<string, unknown>, cameraMoves = true) {
  const frame = document.querySelector<HTMLIFrameElement>("iframe");
  if (frame?.contentWindow) Object.assign(frame.contentWindow, { engine_console_command: async () => "[]" });
  settleEngineWorld(frame);
  act(() => { for (const channel of Channel.instances) if (!channel.closed) channel.onmessage?.({ data: { to: "page", msg } }); });
  if (msg.type === "scene-ready" && cameraMoves) deliver(OVERHEAD_CAMERA_POSE);
}
async function connected() {
  deliver(ready);
  await waitFor(() => expect(rpcs("restoreComposite").length).toBeGreaterThan(0));
  deliver({ type: "rpc-reply", id: rpcs("restoreComposite").at(-1)!.id, ok: true });
  await waitFor(() => expect(screen.getByRole("button", { name: "Run the scene" })).toBeEnabled());
  await editViewRevealed();
  await waitFor(() => expect(rpcs("sceneStats").length).toBeGreaterThan(0));
}
async function captureEdit() {
  deliver({ type: "drag-end", transforms: { "42": { position: { x: 4, y: 1, z: 2 } } } });
  await waitFor(() => expect(rpcs("exportComposite")).toHaveLength(1));
  deliver({ type: "rpc-reply", id: rpcs("exportComposite")[0]!.id, ok: true, result: edited });
  await waitFor(() => expect(records.get("project-a")?.composite).toBe(edited));
}
beforeEach(async () => {
  Channel.instances = []; Channel.sent = []; Channel.runtime = 1; Channel.tick = 3; Channel.broken = false; records.clear();
  vi.stubGlobal("BroadcastChannel", Channel);
  vi.spyOn(sceneCheckpointStore, "read").mockImplementation(async key => records.get(key) ?? null);
  vi.spyOn(sceneCheckpointStore, "write").mockImplementation(async record => { records.set(record.key, record); });
  window.localStorage.setItem("eui-camera-hint-dismissed", "1");
  await useEditorClock();
});
afterEach(() => { vi.useRealTimers(); cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); window.localStorage.clear(); });
async function debouncedCaptureChecked(edit: Record<string, unknown>) {
  const stats = rpcs("sceneStats").length;
  deliver(edit);
  await act(async () => { await vi.advanceTimersByTimeAsync(RECOVERY_DELAY_MS); });
  expect(rpcs("sceneStats").length).toBeGreaterThan(stats);
}

it("rehydrates acknowledged engine edits after iframe failure and after a full workspace reload", async () => {
  const first = render(<DeWorkspace viewportSrc={viewportSrc} rawComposite={saved} recoveryKey="project-a" />);
  await connected();
  await captureEdit();
  fireEvent.load(screen.getByTitle("Scene viewport"));
  deliver(ready);
  await waitFor(() => expect(rpcs("restoreComposite")).toHaveLength(2));
  expect(rpcs("restoreComposite")[1]!.args).toEqual([edited]);
  deliver({ type: "rpc-reply", id: rpcs("restoreComposite")[1]!.id, ok: true });
  expect(await screen.findByText(/Recovered your unsaved scene changes/)).toBeVisible();
  first.unmount();
  render(<DeWorkspace viewportSrc={viewportSrc} rawComposite={saved} recoveryKey="project-a" />);
  deliver(ready);
  await waitFor(() => expect(rpcs("restoreComposite")).toHaveLength(3));
  expect(rpcs("restoreComposite")[2]!.args).toEqual([edited]);
});

it("a deliberately different saved input and a different project take precedence over old recovery", async () => {
  records.set("project-a", { version: 1, key: "project-a", input: saved, composite: edited, savedAt: 1 });
  const newer = '{"components":[],"revision":2}';
  const first = render(<DeWorkspace viewportSrc={viewportSrc} rawComposite={newer} recoveryKey="project-a" />);
  deliver(ready);
  await waitFor(() => expect(rpcs("restoreComposite")[0]!.args).toEqual([newer]));
  first.unmount();
  render(<DeWorkspace viewportSrc={viewportSrc} rawComposite={saved} recoveryKey="project-b" />);
  deliver(ready);
  await waitFor(() => expect(rpcs("restoreComposite")[1]!.args).toEqual([saved]));
});

it("preserves the pre-Play authored snapshot and cannot checkpoint simulation edits", async () => {
  render(<DeWorkspace viewportSrc={viewportSrc} rawComposite={saved} recoveryKey="project-a" />);
  await connected();
  fireEvent.keyDown(window, { key: "F5" });
  deliver({ type: "rpc-reply", id: rpcs("exportComposite")[0]!.id, ok: true, result: edited });
  await waitFor(() => expect(rpcs("setPlayback")).toHaveLength(1));
  expect(records.get("project-a")?.composite).toBe(edited);
  deliver({ type: "rpc-reply", id: rpcs("setPlayback")[0]!.id, ok: true });
  await waitFor(() => expect(screen.getByRole("button", { name: "Stop the preview and return to editing" })).toBeEnabled());
  deliver({ type: "drag-end", transforms: {} });
  fireEvent.load(screen.getByTitle("Scene viewport"));
  deliver(ready);
  await waitFor(() => expect(rpcs("restoreComposite")).toHaveLength(2));
  expect(rpcs("restoreComposite")[1]!.args).toEqual([edited]);
});

it("rejects an export from the disconnected engine instead of overwriting a newer checkpoint", async () => {
  render(<DeWorkspace viewportSrc={viewportSrc} rawComposite={saved} recoveryKey="project-a" />);
  await connected();
  deliver({ type: "drag-end", transforms: {} });
  await waitFor(() => expect(rpcs("exportComposite")).toHaveLength(1));
  const stale = rpcs("exportComposite")[0]!.id;
  fireEvent.load(screen.getByTitle("Scene viewport"));
  deliver({ type: "rpc-reply", id: stale, ok: true, result: edited });
  expect(records.size).toBe(0);
  deliver(ready);
  await waitFor(() => expect(rpcs("restoreComposite")).toHaveLength(2));
  expect(rpcs("restoreComposite")[1]!.args).toEqual([saved]);
});

it("hydrates the latest authored checkpoint after the agent announces an SDK runtime replacement", async () => {
  render(<DeWorkspace viewportSrc={viewportSrc} rawComposite={saved} recoveryKey="project-a" />);
  await connected();
  await captureEdit();
  deliver(ready);
  await waitFor(() => expect(Channel.sent.filter(({ msg }) => msg.type === "init")).toHaveLength(2));
  deliver(ready);
  await waitFor(() => expect(rpcs("restoreComposite")).toHaveLength(2));
  expect(rpcs("restoreComposite")[1]!.args).toEqual([edited]);
});

it("reveals the editor view after an SDK runtime re-announce when the settled camera posts no new pose", async () => {
  render(<DeWorkspace viewportSrc={viewportSrc} rawComposite={saved} recoveryKey="project-a" />);
  await connected();
  deliver(ready, false);
  await waitFor(() => expect(Channel.sent.filter(({ msg }) => msg.type === "init")).toHaveLength(2));
  deliver(ready, false);
  await waitFor(() => expect(rpcs("restoreComposite")).toHaveLength(2));
  deliver({ type: "rpc-reply", id: rpcs("restoreComposite")[1]!.id, ok: true });
  await editViewRevealed();
  expect(screen.queryByText(/The editor camera did not become ready/)).toBeNull();
});

it("hydrates an authoritative checkpoint sent immediately before scene-ready while durable storage is slow", async () => {
  let release!: () => void;
  vi.mocked(sceneCheckpointStore.write).mockImplementation(async record => {
    await new Promise<void>(resolve => { release = resolve; });
    records.set(record.key, record);
  });
  render(<DeWorkspace viewportSrc={viewportSrc} rawComposite={saved} recoveryKey="project-a" />);
  await connected();
  deliver({ type: "authoring-checkpoint", composite: edited, runtime: "scene:1" });
  Channel.runtime = 2;
  deliver(ready);
  await waitFor(() => expect(Channel.sent.filter(({ msg }) => msg.type === "init")).toHaveLength(2));
  deliver(ready);
  await waitFor(() => expect(release).toBeTypeOf("function"));
  expect(rpcs("restoreComposite")).toHaveLength(1);
  await act(async () => { release(); });
  await waitFor(() => expect(rpcs("restoreComposite")).toHaveLength(2));
  expect(rpcs("restoreComposite")[1]!.args).toEqual([edited]);
  expect(records.get("project-a")?.composite).toBe(edited);
  expect(rpcs("exportComposite")).toHaveLength(0);
});

it("cannot overwrite authored recovery with a replacement runtime before its ready announcement", async () => {
  render(<DeWorkspace viewportSrc={viewportSrc} rawComposite={saved} recoveryKey="project-a" />);
  await connected();
  await captureEdit();
  Channel.runtime = 2;
  await debouncedCaptureChecked({ type: "drag-end", transforms: {} });
  expect(rpcs("exportComposite")).toHaveLength(1);
  expect(records.get("project-a")?.composite).toBe(edited);
});

it.each(["not started", "broken"])("does not checkpoint a %s runtime even if the editor previously connected", async status => {
  render(<DeWorkspace viewportSrc={viewportSrc} rawComposite={saved} recoveryKey="project-a" />);
  await connected();
  Channel.tick = status === "not started" ? 2 : 3;
  Channel.broken = status === "broken";
  await debouncedCaptureChecked({ type: "drag-end", transforms: {} });
  expect(rpcs("exportComposite")).toHaveLength(0);
  expect(records.size).toBe(0);
  Channel.tick = 3;
  Channel.broken = false;
  await captureEdit();
});

it("retries the same acknowledged snapshot after a transient storage failure even without another edit", async () => {
  vi.mocked(sceneCheckpointStore.write).mockRejectedValueOnce(new Error("temporarily unavailable"));
  render(<DeWorkspace viewportSrc={viewportSrc} rawComposite={saved} recoveryKey="project-a" />);
  await connected();
  deliver({ type: "drag-end", transforms: {} });
  await waitFor(() => expect(rpcs("exportComposite")).toHaveLength(1));
  deliver({ type: "rpc-reply", id: rpcs("exportComposite")[0]!.id, ok: true, result: edited });
  expect(await screen.findByRole("alert")).toHaveTextContent("Automatic recovery could not be saved");
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
  act(() => document.dispatchEvent(new Event("visibilitychange")));
  await waitFor(() => expect(rpcs("exportComposite")).toHaveLength(2));
  deliver({ type: "rpc-reply", id: rpcs("exportComposite")[1]!.id, ok: true, result: edited });
  await waitFor(() => expect(records.get("project-a")?.composite).toBe(edited));
  await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
});

it("warns on durable storage failure but preserves the acknowledged engine edit for Retry", async () => {
  vi.mocked(sceneCheckpointStore.write).mockRejectedValue(new Error("quota"));
  render(<DeWorkspace viewportSrc={viewportSrc} rawComposite={saved} recoveryKey="project-a" />);
  await connected();
  deliver({ type: "drag-end", transforms: {} });
  await waitFor(() => expect(rpcs("exportComposite")).toHaveLength(1));
  deliver({ type: "rpc-reply", id: rpcs("exportComposite")[0]!.id, ok: true, result: edited });
  expect(await screen.findByRole("alert")).toHaveTextContent("Automatic recovery could not be saved");
  fireEvent.load(screen.getByTitle("Scene viewport"));
  deliver(ready);
  await waitFor(() => expect(rpcs("restoreComposite")).toHaveLength(2));
  expect(rpcs("restoreComposite")[1]!.args).toEqual([edited]);
});

it("rebases a recovered upload to the new document before engine hydration and durable checkpointing", async () => {
  const before = `${window.location.origin}/_project/sessions/00000000-0000-4000-8000-000000000001/content`;
  const after = `${window.location.origin}/_project/sessions/00000000-0000-4000-8000-000000000002/content`;
  const assetInput = JSON.stringify({ components: [{ name: "core::GltfContainer", data: { "42": { json: { src: `${before}/contents/upload.glb` } } } }] });
  const assetEdited = assetInput.replace('"components":[', '"components":[{"name":"core::Name","data":{"42":{"json":{"value":"Unsaved model"}}}},');
  const rebasedInput = assetInput.replace(before, after);
  const rebasedEdited = assetEdited.replace(before, after);
  const scopedViewport = `/_play?editorSession=00000000-0000-4000-8000-000000000003&realm=${encodeURIComponent(after.replace(/\/content$/, ""))}`;
  records.set("project-a", { version: 1, key: "project-a", input: assetInput, composite: assetEdited, savedAt: 1 });
  render(<DeWorkspace viewportSrc={scopedViewport} rawComposite={rebasedInput} recoveryKey="project-a" />);
  await waitFor(() => expect(Channel.sent.some(({ msg }) => msg.type === "init")).toBe(true));
  deliver(ready);
  await waitFor(() => expect(rpcs("restoreComposite")[0]!.args).toEqual([rebasedEdited]));
  expect(records.get("project-a")?.composite).toBe(rebasedEdited);
  expect(records.get("project-a")?.input).toBe(rebasedInput);
});

it("retires an in-flight lineage copy when the prepared realm changes the recovery scope", async () => {
  const pending: Array<(value: SceneCheckpoint | null) => void> = [];
  vi.mocked(sceneCheckpointStore.read).mockImplementation(key => key === "previous"
    ? Promise.resolve({ version: 1, key, input: saved, composite: edited, savedAt: 1 })
    : new Promise(resolve => pending.push(resolve)));
  const hook = renderHook(({ viewportSrc }: { viewportSrc: string | null }) => useSceneRecovery({
    key: "current", previousKey: "previous", input: saved, viewportSrc, busRef: { current: null }, enabled: false,
  }), { initialProps: { viewportSrc: null as string | null } });
  const realm = `${window.location.origin}/_project/sessions/00000000-0000-4000-8000-000000000004`;
  hook.rerender({ viewportSrc: `/_play?realm=${encodeURIComponent(realm)}` });
  expect(pending).toHaveLength(2);
  await act(async () => { pending[0]!(null); });
  expect(sceneCheckpointStore.write).not.toHaveBeenCalled();
  await act(async () => { pending[1]!(null); });
  await act(async () => { expect(await hook.result.current.restore()).toBe(edited); });
  expect(sceneCheckpointStore.write).toHaveBeenCalledTimes(1);
  expect(records.get("current")?.composite).toBe(edited);
});

it.each([new Error("quota"), new ProjectUploadUnavailableError()])("keeps acknowledged authored work and dirty state when recovering an asset fails: %s", async error => {
  const origin = window.location.origin;
  const before = `${origin}/_project/sessions/00000000-0000-4000-8000-000000000001/content/contents/model.glb`;
  const composite = JSON.stringify({ components: [{ name: "core::GltfContainer", data: { "42": { json: { src: before } } } }] });
  records.set("project-a", { version: 1, key: "project-a", input: saved, composite, savedAt: 1 });
  vi.stubGlobal("caches", { has: async () => true, open: async () => { throw error; } });
  const onRecovered = vi.fn();
  const realm = `${origin}/_project/sessions/00000000-0000-4000-8000-000000000005`;
  const hook = renderHook(() => useSceneRecovery({
    key: "project-a", input: saved, viewportSrc: `/_play?realm=${encodeURIComponent(realm)}`,
    busRef: { current: null }, enabled: false, onRecovered,
  }));
  await act(async () => { expect(await hook.result.current.restore()).toBe(composite); });
  expect(hook.result.current.error).toContain(error instanceof ProjectUploadUnavailableError ? error.message : "uploaded files could not be restored");
  expect(hook.result.current.notice).toContain("Recovered your unsaved scene changes");
  expect(onRecovered).toHaveBeenCalledOnce();
  expect(records.get("project-a")?.composite).toBe(composite);
  expect(sceneCheckpointStore.write).not.toHaveBeenCalled();
  const warning = hook.result.current.error;
  await act(async () => { await hook.result.current.preserve(composite); });
  expect(hook.result.current.error).toBe(warning);
  act(() => hook.result.current.dismiss());
  expect(hook.result.current.error).toBeNull();
});

async function authoringHandle() {
  let capture: CaptureAuthoredScene | null = null;
  const onAuthoringCaptureReady = (value: CaptureAuthoredScene | null) => { capture = value; };
  const props = { viewportSrc, rawComposite: saved, recoveryKey: "project-a", onAuthoringCaptureReady };
  const view = render(<DeWorkspace {...props} />);
  await connected();
  return { capture: () => { if (!capture) throw new Error("Authoring capture not attached"); return capture(); }, busy: () => view.rerender(<DeWorkspace {...props} operationsBusy />) };
}

it("shares one authored checkpoint across concurrent unchanged capture callers", async () => {
  const handle = await authoringHandle();
  const pending = Promise.all([handle.capture(), handle.capture(), handle.capture()]);
  await waitFor(() => expect(rpcs("exportComposite")).toHaveLength(1));
  deliver({ type: "rpc-reply", id: rpcs("exportComposite")[0]!.id, ok: true, result: edited });
  await act(async () => { expect(await pending).toEqual([edited, edited, edited]); });
  expect(rpcs("exportComposite")).toHaveLength(1);
  expect(sceneCheckpointStore.write).toHaveBeenCalledTimes(1);
});

it("queues a newer authored revision without cancelling an earlier explicit capture", async () => {
  const handle = await authoringHandle();
  const first = handle.capture();
  await waitFor(() => expect(rpcs("exportComposite")).toHaveLength(1));
  deliver({ type: "drag-end", transforms: {} });
  const second = handle.capture();
  expect(rpcs("exportComposite")).toHaveLength(1);
  deliver({ type: "rpc-reply", id: rpcs("exportComposite")[0]!.id, ok: true, result: saved });
  await act(async () => { expect(await first).toBe(saved); });
  await waitFor(() => expect(rpcs("exportComposite")).toHaveLength(2));
  deliver({ type: "rpc-reply", id: rpcs("exportComposite")[1]!.id, ok: true, result: edited });
  await act(async () => { expect(await second).toBe(edited); });
  expect(records.get("project-a")?.composite).toBe(edited);
  expect(sceneCheckpointStore.write).toHaveBeenCalledTimes(2);
});

it("checkpoints the exact immediate save export before a rebuild, including while that save makes operations busy", async () => {
  const handle = await authoringHandle();
  handle.busy();
  const pending = handle.capture();
  await waitFor(() => expect(rpcs("exportComposite")).toHaveLength(1));
  deliver({ type: "rpc-reply", id: rpcs("exportComposite")[0]!.id, ok: true, result: edited });
  await act(async () => { expect(await pending).toBe(edited); });
  expect(records.get("project-a")?.composite).toBe(edited);
  Channel.runtime = 2;
  deliver(ready);
  await waitFor(() => expect(Channel.sent.filter(({ msg }) => msg.type === "init")).toHaveLength(2));
  deliver(ready);
  await waitFor(() => expect(rpcs("restoreComposite")).toHaveLength(2));
  expect(rpcs("restoreComposite")[1]!.args).toEqual([edited]);
});

it("retains the immediate save snapshot in the live controller when browser quota fails", async () => {
  const handle = await authoringHandle();
  vi.mocked(sceneCheckpointStore.write).mockRejectedValue(new Error("quota"));
  const pending = handle.capture();
  await waitFor(() => expect(rpcs("exportComposite")).toHaveLength(1));
  deliver({ type: "rpc-reply", id: rpcs("exportComposite")[0]!.id, ok: true, result: edited });
  await act(async () => { expect(await pending).toBe(edited); });
  expect(await screen.findByRole("alert")).toHaveTextContent("Automatic recovery could not be saved");
  expect(records.size).toBe(0);
  fireEvent.load(screen.getByTitle("Scene viewport"));
  deliver(ready);
  await waitFor(() => expect(rpcs("restoreComposite")).toHaveLength(2));
  expect(rpcs("restoreComposite")[1]!.args).toEqual([edited]);
});

it.each(["invalid export", "replacement runtime"])("rejects a save barrier with %s without replacing the authored checkpoint", async cause => {
  const handle = await authoringHandle();
  const pending = expect(handle.capture()).rejects.toThrow(cause === "invalid export" ? /invalid authored/ : /changed during capture/);
  await waitFor(() => expect(rpcs("exportComposite")).toHaveLength(1));
  if (cause === "replacement runtime") Channel.runtime = 2;
  deliver({ type: "rpc-reply", id: rpcs("exportComposite")[0]!.id, ok: true, result: cause === "invalid export" ? "invalid" : edited });
  await pending;
  expect(sceneCheckpointStore.write).not.toHaveBeenCalled();
});

it("prevents an older periodic export from arriving after the explicit save snapshot", async () => {
  const handle = await authoringHandle();
  deliver({ type: "drag-end", transforms: {} });
  await waitFor(() => expect(rpcs("exportComposite")).toHaveLength(1));
  const old = rpcs("exportComposite")[0]!.id;
  const pending = handle.capture();
  await waitFor(() => expect(rpcs("exportComposite")).toHaveLength(2));
  deliver({ type: "rpc-reply", id: rpcs("exportComposite")[1]!.id, ok: true, result: edited });
  await act(async () => { expect(await pending).toBe(edited); });
  deliver({ type: "rpc-reply", id: old, ok: true, result: saved });
  await act(async () => { await Promise.resolve(); });
  expect(records.get("project-a")?.composite).toBe(edited);
  expect(sceneCheckpointStore.write).toHaveBeenCalledTimes(1);
});

it("rejects a file-save barrier if the engine disconnects while its checkpoint commits", async () => {
  const handle = await authoringHandle();
  let releaseWrite: (() => void) | undefined;
  vi.mocked(sceneCheckpointStore.write).mockImplementation(async record => {
    await new Promise<void>(resolve => { releaseWrite = resolve; });
    records.set(record.key, record);
  });
  const pending = expect(handle.capture()).rejects.toThrow(/retain|changed/);
  await waitFor(() => expect(rpcs("exportComposite")).toHaveLength(1));
  deliver({ type: "rpc-reply", id: rpcs("exportComposite")[0]!.id, ok: true, result: edited });
  await waitFor(() => expect(releaseWrite).toBeTypeOf("function"));
  fireEvent.load(screen.getByTitle("Scene viewport"));
  await act(async () => { releaseWrite!(); await pending; });
  expect(records.get("project-a")?.composite).toBe(edited);
});

it("refuses to capture Play simulation state for a file save", async () => {
  const handle = await authoringHandle();
  fireEvent.keyDown(window, { key: "F5" });
  deliver({ type: "rpc-reply", id: rpcs("exportComposite")[0]!.id, ok: true, result: edited });
  await waitFor(() => expect(rpcs("setPlayback")).toHaveLength(1));
  deliver({ type: "rpc-reply", id: rpcs("setPlayback")[0]!.id, ok: true });
  await waitFor(() => expect(screen.getByRole("button", { name: "Stop the preview and return to editing" })).toBeEnabled());
  await expect(handle.capture()).rejects.toThrow(/connected and stopped/);
  expect(rpcs("exportComposite")).toHaveLength(1);
  expect(records.get("project-a")?.composite).toBe(edited);
});
