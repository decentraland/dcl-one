import { afterEach, expect, test, vi } from "vitest";

import { stagePersistedLocalProject } from "./local-scene";
import { handleStore } from "./handle-store";
import { populateProjectRealm } from "./project-realm";
import { readDirectoryFiles } from "./disk";
import { projectScopeForId } from "@ui/editor/project-cache-name";

vi.mock("./handle-store", () => ({
  handleStore: { get: vi.fn() },
  ensureHandlePermission: vi.fn(async () => true),
}));
vi.mock("./project-realm", () => ({ populateProjectRealm: vi.fn(async () => ({ ok: true })) }));
vi.mock("./disk", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./disk")>()),
  readDirectoryFiles: vi.fn(),
}));

afterEach(() => {
  vi.clearAllMocks();
  vi.unstubAllGlobals();
});

function stubRealm(staged: string[]) {
  vi.stubGlobal("caches", { open: async () => ({ match: async (key: string) => (staged.includes(key) ? new Response("{}") : undefined) }) });
}

test("a realm that Import already staged is left alone", async () => {
  const scope = projectScopeForId("11111111-1111-4111-8111-111111111111");
  stubRealm([`${scope.realmBase}/about`]);
  await stagePersistedLocalProject("cov-a", scope);
  expect(handleStore.get).not.toHaveBeenCalled();
  expect(populateProjectRealm).not.toHaveBeenCalled();
});

test("an unstaged saved project is staged into this editor's realm from its folder", async () => {
  const scope = projectScopeForId("22222222-2222-4222-8222-222222222222");
  stubRealm([]);
  const files = { "scene.json": new File(['{"display":{"title":"Cov A"}}'], "scene.json"), "main.composite": new File(["{}"], "main.composite") };
  vi.mocked(handleStore.get).mockResolvedValue({ kind: "directory", name: "cov-a" } as unknown as FileSystemDirectoryHandle);
  vi.mocked(readDirectoryFiles).mockResolvedValue(files);
  await stagePersistedLocalProject("cov-a", scope);
  expect(handleStore.get).toHaveBeenCalledWith("cov-a");
  expect(populateProjectRealm).toHaveBeenCalledWith(files, '{"display":{"title":"Cov A"}}', expect.objectContaining({ scope }));
});

test("a project whose folder is no longer linked says how to recover", async () => {
  const scope = projectScopeForId("33333333-3333-4333-8333-333333333333");
  stubRealm([]);
  vi.mocked(handleStore.get).mockResolvedValue(null);
  await expect(stagePersistedLocalProject("gone", scope)).rejects.toThrow(/Import Scene/);
  expect(populateProjectRealm).not.toHaveBeenCalled();
});
