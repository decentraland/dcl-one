import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { editorBusChannelFromViewportSrc, type EditorBus } from "../editor-bus";
import { sceneCheckpointStore, type SceneCheckpoint } from "../scene-recovery";
import { useSceneRecovery } from "./useSceneRecovery";

class Channel {
  static instances: Channel[] = [];
  onmessage: ((event: { data: unknown }) => void) | null = null;
  closed = false;
  constructor(readonly name: string) { Channel.instances.push(this); }
  close() { this.closed = true; }
}
const input = '{"components":[]}';
const authored = '{"components":[{"name":"core::Name","data":{"512":{"json":{"value":"Peer edit"}}}}]}';
const viewportSrc = "/_play?editorSession=00000000-0000-4000-8000-000000000001&realm=project";
const channelName = editorBusChannelFromViewportSrc(viewportSrc)!;
const stats = "hash: scene\ninstance: 1\ntick: 3\nbroken: false";
const records = new Map<string, SceneCheckpoint>();
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}
function checkpoint(composite = authored, runtime = "scene:1", name = channelName) {
  act(() => {
    for (const channel of Channel.instances) if (!channel.closed && channel.name === name) channel.onmessage?.({ data: {
      to: "page", msg: { type: "authoring-checkpoint", composite, runtime },
    } });
  });
}
function workspace(rpc = vi.fn(async () => stats), exportComposite = vi.fn(async () => input)) {
  const bus = { rpc, exportComposite } as unknown as EditorBus;
  const busRef = { current: bus };
  return renderHook(({ key, enabled, playing }: { key: string; enabled: boolean; playing: boolean }) => useSceneRecovery({
    key, input, viewportSrc, busRef, enabled, playing,
  }), { initialProps: { key: "project-a", enabled: true, playing: false } });
}
beforeEach(() => {
  records.clear(); Channel.instances = [];
  vi.stubGlobal("BroadcastChannel", Channel);
  vi.spyOn(sceneCheckpointStore, "read").mockImplementation(async key => records.get(key) ?? null);
  vi.spyOn(sceneCheckpointStore, "write").mockImplementation(async record => { records.set(record.key, record); });
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });

it("queues before reconnect even while runtime validation and durable storage are both pending", async () => {
  const runtime = deferred<string>();
  const storage = deferred<void>();
  vi.mocked(sceneCheckpointStore.write).mockImplementation(async record => { await storage.promise; records.set(record.key, record); });
  const hook = workspace(vi.fn(() => runtime.promise));
  checkpoint();
  hook.rerender({ key: "project-a", enabled: false, playing: false });
  let restored = false;
  const restore = hook.result.current.restore().then(value => { restored = true; return value; });
  await act(async () => { runtime.resolve(stats); });
  expect(sceneCheckpointStore.write).toHaveBeenCalledOnce();
  expect(restored).toBe(false);
  await act(async () => { storage.resolve(); expect(await restore).toBe(authored); });
  expect(records.get("project-a")?.composite).toBe(authored);
});

it("accepts a previously authored checkpoint arriving after transport reconnect disables polling", async () => {
  const hook = workspace();
  hook.rerender({ key: "project-a", enabled: false, playing: false });
  checkpoint();
  await act(async () => { expect(await hook.result.current.restore()).toBe(authored); });
});

it("never accepts an authoritative event during Play, including paused Play", async () => {
  const hook = workspace();
  checkpoint();
  await act(async () => { expect(await hook.result.current.restore()).toBe(authored); });
  hook.rerender({ key: "project-a", enabled: false, playing: true });
  checkpoint(input);
  await act(async () => { expect(await hook.result.current.restore()).toBe(authored); });
  expect(sceneCheckpointStore.write).toHaveBeenCalledOnce();
});

it("rejects another scoped channel, runtime, and invalid composite", async () => {
  const hook = workspace();
  checkpoint(authored, "scene:1", channelName.replace(/1$/, "2"));
  checkpoint(authored, "scene:2");
  checkpoint("invalid");
  await act(async () => { expect(await hook.result.current.restore()).toBe(input); });
  expect(sceneCheckpointStore.write).not.toHaveBeenCalled();
});

it("does not let pending runtime validation or a late event write a superseded project controller", async () => {
  const runtime = deferred<string>();
  const hook = workspace(vi.fn(() => runtime.promise));
  checkpoint();
  hook.rerender({ key: "project-b", enabled: false, playing: false });
  checkpoint();
  await act(async () => { runtime.resolve(stats); expect(await hook.result.current.restore()).toBe(input); });
  expect(sceneCheckpointStore.write).not.toHaveBeenCalled();
});

it("invalidates a prior explicit export so it cannot replace a newer acknowledged checkpoint", async () => {
  const exported = deferred<string>();
  const exportComposite = vi.fn(() => exported.promise);
  const hook = workspace(vi.fn(async () => stats), exportComposite);
  const capture = hook.result.current.captureAuthoring();
  const rejected = expect(capture).rejects.toThrow("changed while capturing");
  await waitFor(() => expect(exportComposite).toHaveBeenCalledOnce());
  checkpoint();
  await act(async () => { exported.resolve(input); await rejected; });
  await act(async () => { expect(await hook.result.current.restore()).toBe(authored); });
  expect(records.get("project-a")?.composite).toBe(authored);
});

it("a checkpoint that repeats the acknowledged contents does not fail an explicit capture", async () => {
  const exported = deferred<string>();
  const exportComposite = vi.fn(() => exported.promise);
  const hook = workspace(vi.fn(async () => stats), exportComposite);
  checkpoint();
  const capture = hook.result.current.captureAuthoring();
  await waitFor(() => expect(exportComposite).toHaveBeenCalledOnce());
  checkpoint();
  await act(async () => { exported.resolve(authored); expect(await capture).toBe(authored); });
  expect(records.get("project-a")?.composite).toBe(authored);
});

it("retains an acknowledged peer edit on quota failure and warns without discarding it on reconnect", async () => {
  vi.mocked(sceneCheckpointStore.write).mockRejectedValue(new Error("quota"));
  const hook = workspace();
  checkpoint();
  hook.rerender({ key: "project-a", enabled: false, playing: false });
  await act(async () => { expect(await hook.result.current.restore()).toBe(authored); });
  expect(hook.result.current.error).toContain("Automatic recovery could not be saved");
  expect(records.size).toBe(0);
});

it("does not clear a real persistence warning when a different runtime's checkpoint is rejected", async () => {
  vi.mocked(sceneCheckpointStore.write).mockRejectedValue(new Error("quota"));
  const hook = workspace();
  checkpoint();
  await act(async () => { expect(await hook.result.current.restore()).toBe(authored); });
  expect(hook.result.current.error).toContain("Automatic recovery could not be saved");
  checkpoint(input, "scene:2");
  await act(async () => { await Promise.resolve(); await Promise.resolve(); });
  expect(hook.result.current.error).toContain("Automatic recovery could not be saved");
  await act(async () => { expect(await hook.result.current.restore()).toBe(authored); });
  expect(sceneCheckpointStore.write).toHaveBeenCalledOnce();
});
