import { afterEach, expect, it, vi } from "vitest";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { saveTextFile, writeTextHandle } from "./disk";

const directories: string[] = [];
afterEach(async () => { vi.unstubAllGlobals(); await Promise.all(directories.splice(0).map(directory => rm(directory, { recursive: true, force: true }))); });

async function physicalFile() {
  const directory = await mkdtemp(join(tmpdir(), "editor-disk-conflict-"));
  directories.push(directory);
  const path = join(directory, "main.composite");
  await writeFile(path, "original");
  let duringWrite: (() => Promise<void>) | undefined;
  let staged = "";
  const writer = { write: vi.fn(async (text: string) => { staged = text; await duringWrite?.(); }), close: vi.fn(async () => { await writeFile(path, staged); }), abort: vi.fn(async () => {}) };
  const handle = { name: "main.composite", getFile: async () => new File([await readFile(path)], "main.composite"), createWritable: vi.fn(async () => writer) } as unknown as FileSystemFileHandle;
  return { path, handle, writer, intervene(action: () => Promise<void>) { duringWrite = action; } };
}

it("writes and reopens the expected physical bytes", async () => {
  const file = await physicalFile();
  await writeTextHandle(file.handle, "authored scene", "original");
  expect(await readFile(file.path, "utf8")).toBe("authored scene");
  await writeTextHandle(file.handle, "second scene", "authored scene");
  expect(await (await file.handle.getFile()).text()).toBe("second scene");
});

it("does not open a writer when the saved scene changed externally", async () => {
  const file = await physicalFile();
  await writeFile(file.path, "external");
  await expect(writeTextHandle(file.handle, "unsaved browser scene", "original")).rejects.toThrow("changed outside");
  expect(file.handle.createWritable).not.toHaveBeenCalled();
  expect(await readFile(file.path, "utf8")).toBe("external");
});

it("aborts a staged write when external changes arrive before close", async () => {
  const file = await physicalFile();
  file.intervene(() => writeFile(file.path, "external during write"));
  await expect(writeTextHandle(file.handle, "browser scene", "original")).rejects.toThrow("changed outside");
  expect(file.writer.abort).toHaveBeenCalledOnce();
  expect(file.writer.close).not.toHaveBeenCalled();
  expect(await readFile(file.path, "utf8")).toBe("external during write");
});

it("does not convert an observed conflict into a download or another save picker", async () => {
  const file = await physicalFile();
  const picker = vi.fn();
  vi.stubGlobal("window", { showSaveFilePicker: picker });
  await writeFile(file.path, "external");
  await expect(saveTextFile("main.composite", "browser scene", file.handle, { expectedText: "original" })).rejects.toThrow("changed outside");
  expect(picker).not.toHaveBeenCalled();
});

it("cancellation after staging aborts rather than committing bytes", async () => {
  const file = await physicalFile(), controller = new AbortController();
  file.intervene(async () => { controller.abort(); });
  await expect(writeTextHandle(file.handle, "cancelled scene", "original", controller.signal)).rejects.toMatchObject({ name: "AbortError" });
  expect(file.writer.close).not.toHaveBeenCalled();
  expect(await readFile(file.path, "utf8")).toBe("original");
});
