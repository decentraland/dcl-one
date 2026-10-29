import { expect, it, vi } from "vitest";
import { createSceneRecovery, matchingCheckpoint, type SceneCheckpoint, type SceneCheckpointStore } from "./scene-recovery";
import { canonicalProjectComposite, projectScopeForId } from "./project-cache-name";

const input = '{"components":[]}';
const authored = '{"components":[{"name":"Transform","data":{"42":{"json":{"position":{"x":4,"y":1,"z":2}}}}}]}';
const record = (composite = authored): SceneCheckpoint => ({ version: 1, key: "project-a", input, composite, savedAt: 1 });
const store = (value: SceneCheckpoint | null = null): SceneCheckpointStore => ({ read: vi.fn(async () => value), write: vi.fn(async () => {}) });

it("restores only a validated checkpoint belonging to this project and saved input revision", async () => {
  expect(await createSceneRecovery("project-a", input, store(record())).restore()).toBe(authored);
  expect(await createSceneRecovery("project-b", input, store(record())).restore()).toBe(input);
  const newlySaved = '{"components":[],"revision":2}';
  expect(await createSceneRecovery("project-a", newlySaved, store(record())).restore()).toBe(newlySaved);
  expect(matchingCheckpoint({ ...record(), composite: "corrupt" }, "project-a", input)).toBeNull();
  expect(matchingCheckpoint({ ...record(), version: 2 }, "project-a", input)).toBeNull();
});

it("waits for durable commit, serializes newer snapshots, and never writes a retired export", async () => {
  let release!: () => void;
  const storage = store();
  const saved: string[] = [];
  storage.write = vi.fn(async snapshot => { saved.push(snapshot.composite); if (saved.length === 1) await new Promise<void>(resolve => { release = resolve; }); });
  const recovery = createSceneRecovery("project-a", input, storage);
  const first = recovery.checkpoint(authored);
  await vi.waitFor(() => expect(saved).toEqual([authored]));
  const newer = '{"components":[],"revision":3}';
  const second = recovery.checkpoint(newer);
  expect(saved).toHaveLength(1);
  release();
  await Promise.all([first, second]);
  expect(saved).toEqual([authored, newer]);
  expect(await recovery.restore()).toBe(newer);
  recovery.retire();
  expect(await recovery.checkpoint(input)).toBe(false);
  expect(saved).toHaveLength(2);
});

it("a reconnect waits for already queued authored checkpoints before choosing its restore state", async () => {
  let release!: () => void;
  const storage = store();
  const saved: string[] = [];
  storage.write = vi.fn(async snapshot => {
    saved.push(snapshot.composite);
    if (saved.length === 1) await new Promise<void>(resolve => { release = resolve; });
  });
  const recovery = createSceneRecovery("project-a", input, storage);
  const first = recovery.checkpoint(authored);
  await vi.waitFor(() => expect(saved).toEqual([authored]));
  const newest = '{"components":[],"revision":4}';
  const second = recovery.checkpoint(newest);
  let restored = false;
  const reconnect = recovery.restore().then(value => { restored = true; return value; });
  await Promise.resolve();
  await Promise.resolve();
  expect(restored).toBe(false);
  release();
  await Promise.all([first, second]);
  expect(await reconnect).toBe(newest);
});

it("retains acknowledged work in memory on storage failure, reports failure, then retries persistence", async () => {
  const storage = store();
  const write = vi.fn().mockRejectedValueOnce(new Error("quota")).mockResolvedValue(undefined);
  storage.write = write;
  const recovery = createSceneRecovery("project-a", input, storage);
  await expect(recovery.checkpoint(authored)).rejects.toThrow("quota");
  expect(await recovery.restore()).toBe(authored);
  await expect(recovery.checkpoint(authored)).resolves.toBe(true);
  expect(write).toHaveBeenCalledTimes(2);
});

it("does not overwrite the last checkpoint with invalid exports or superseded generations", async () => {
  const storage = store(record());
  const recovery = createSceneRecovery("project-a", input, storage);
  await expect(recovery.checkpoint("not a composite")).rejects.toThrow("invalid recovery snapshot");
  await expect(recovery.checkpoint(input, () => false)).resolves.toBe(false);
  expect(storage.write).not.toHaveBeenCalled();
  expect(await recovery.restore()).toBe(authored);
});

it("forks persisted recovery into a new document key so duplicate tabs and repeated reloads are independent", async () => {
  const records = new Map<string, SceneCheckpoint>([["project-a", record()]]);
  const storage = { read: async (key: string) => records.get(key) ?? null, write: async (value: SceneCheckpoint) => { records.set(value.key, value); } };
  const first = createSceneRecovery("runtime-b", input, storage, "project-a");
  expect(await first.restore()).toBe(authored);
  const second = createSceneRecovery("runtime-c", input, storage, "runtime-b");
  expect(await second.restore()).toBe(authored);
  await first.checkpoint(input);
  expect(records.get("runtime-c")?.composite).toBe(authored);
  expect(records.get("project-a")?.composite).toBe(authored);
  expect(records.get("runtime-b")?.composite).toBe(input);
});

it("matches saved input across asset namespace forks without accepting a changed authored revision", async () => {
  const before = projectScopeForId("00000000-0000-4000-8000-000000000001");
  const after = projectScopeForId("00000000-0000-4000-8000-000000000002");
  const saved = JSON.stringify({ components: [{ name: "core::GltfContainer", data: { "42": { json: { src: `${before.contentBase}/contents/model.glb` } } } }] });
  const rebased = saved.replace(before.contentBase, after.contentBase);
  const storage = store({ ...record(), input: saved });
  const normalize = (value: string | null) => canonicalProjectComposite(value, after);
  expect(await createSceneRecovery("project-a", rebased, storage, undefined, () => true, normalize).restore()).toBe(authored);
  const changed = rebased.replace("model.glb", "different.glb");
  expect(await createSceneRecovery("project-a", changed, storage, undefined, () => true, normalize).restore()).toBe(changed);
  const records = new Map<string, SceneCheckpoint>([["project-a", { ...record(), input: saved }]]);
  const forkStore = { read: async (key: string) => records.get(key) ?? null, write: async (value: SceneCheckpoint) => { records.set(value.key, value); } };
  expect(await createSceneRecovery("runtime-b", rebased, forkStore, "project-a", () => true, normalize).restore()).toBe(authored);
  expect(records.get("runtime-b")?.input).toBe(rebased);
  expect(records.get("project-a")?.input).toBe(saved);
  expect(matchingCheckpoint({ ...record(), input: undefined }, "project-a", null, normalize)).toBeNull();
});
