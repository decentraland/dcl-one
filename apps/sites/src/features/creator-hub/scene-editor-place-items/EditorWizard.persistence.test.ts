import { expect, test, vi } from "vitest";
import type { SdkProjectConnection } from "@/data/lib/fs/sdk-project";
import { captureAuthoredForPersistence, sdkFileProject } from "./EditorWizard";

test("persistence waits for the live controller's exact captured snapshot", async () => {
  let release!: (value: string) => void;
  const writes: string[] = [];
  const capture = vi.fn(() => new Promise<string>(resolve => { release = resolve; }));
  const pending = captureAuthoredForPersistence(capture).then(value => { writes.push(value); });
  await Promise.resolve();
  expect(writes).toEqual([]);
  release("acknowledged authored scene");
  await pending;
  expect(writes).toEqual(["acknowledged authored scene"]);
});

test("unavailable or retired operations cannot proceed to file writes", async () => {
  await expect(captureAuthoredForPersistence(null)).rejects.toThrow("not ready to preserve");
  const controller = new AbortController();
  let release!: (value: string) => void;
  const capture = vi.fn(() => new Promise<string>(resolve => { release = resolve; }));
  const pending = captureAuthoredForPersistence(capture, controller.signal);
  controller.abort();
  release("old scene");
  await expect(pending).rejects.toMatchObject({ name: "AbortError" });
  const never = vi.fn(async () => "not called");
  await expect(captureAuthoredForPersistence(never, controller.signal)).rejects.toMatchObject({ name: "AbortError" });
  expect(never).not.toHaveBeenCalled();
});

test("SDK code write, delete and independent file sessions capture before triggering rebuild", async () => {
  const events: string[] = [];
  const connection = {
    url: "http://sdk.test",
    write: async (path: string) => { events.push(`write:${path}`); },
    remove: async (path: string) => { events.push(`remove:${path}`); },
    createFileSession: () => connection,
  } as unknown as SdkProjectConnection;
  const project = sdkFileProject(connection, async () => { events.push("checkpoint"); });
  await project.write("src/index.ts", "code");
  await project.remove!("src/old.ts");
  await project.createFileSession!().write("src/other.ts", "more code");
  expect(events).toEqual(["checkpoint", "write:src/index.ts", "checkpoint", "remove:src/old.ts", "checkpoint", "write:src/other.ts"]);
});

test("failed authored capture prevents an SDK file mutation", async () => {
  const write = vi.fn(), remove = vi.fn();
  const connection = { url: "http://sdk.test", write, remove } as unknown as SdkProjectConnection;
  const project = sdkFileProject(connection, async () => { throw new Error("The scene reconnected"); });
  await expect(project.write("src/index.ts", "code")).rejects.toThrow("reconnected");
  await expect(project.remove!("src/old.ts")).rejects.toThrow("reconnected");
  expect(write).not.toHaveBeenCalled();
  expect(remove).not.toHaveBeenCalled();
});

test("SDK asset writes and deletes preserve authoring before triggering rebuild", async () => {
  const events: string[] = [];
  const connection = {
    url: "http://sdk.test",
    assets: {
      write: async (path: string) => { events.push(`asset-write:${path}`); return "revision"; },
      remove: async (path: string) => { events.push(`asset-remove:${path}`); },
    },
  } as unknown as SdkProjectConnection;
  const project = sdkFileProject(connection, async () => { events.push("checkpoint"); });
  await project.assets!.write("audio/sound.wav", new Uint8Array([1]).buffer);
  await project.assets!.remove("images/old.png", "revision");
  expect(events).toEqual(["checkpoint", "asset-write:audio/sound.wav", "checkpoint", "asset-remove:images/old.png"]);
  const blocked = sdkFileProject(connection, async () => { throw new Error("Capture failed"); });
  await expect(blocked.assets!.write("audio/sound.wav", new Uint8Array([2]).buffer)).rejects.toThrow("Capture failed");
  await expect(blocked.assets!.remove("images/old.png", "revision")).rejects.toThrow("Capture failed");
  expect(events).toHaveLength(4);
});
