import { renderToString } from "react-dom/server";
import { afterEach, expect, test, vi } from "vitest";
import DeWorkspace from "@ui/editor/pages/DeWorkspace";
import type { DeWorkspaceCode } from "@ui/editor/types";
import { emptySeed } from "@/data/lib/catalyst/creator-hub/scene-editor";
import { handleStore } from "@/data/lib/fs/handle-store";
import { currentProjectScope, projectScopeForId } from "@ui/editor/project-cache-name";
import EditorWizard, { editorPersistenceKey } from "./EditorWizard";

vi.mock("@ui/editor/pages/DeWorkspace", () => ({ default: vi.fn(() => null) }));
vi.mock("@ui/editor/project-cache-name", async importOriginal => ({
  ...await importOriginal<typeof import("@ui/editor/project-cache-name")>(),
  currentProjectScope: vi.fn(),
}));
vi.mock("@/data/lib/fs/handle-store", async importOriginal => ({
  ...await importOriginal<typeof import("@/data/lib/fs/handle-store")>(),
  handleStore: { get: vi.fn(), getMeta: vi.fn(), putMeta: vi.fn() },
}));
afterEach(() => { vi.resetAllMocks(); vi.unstubAllGlobals(); });

const a = "00000000-0000-4000-8000-000000000011";
const b = "00000000-0000-4000-8000-000000000012";

test("unslugged projects use stable scope identity, preserving explicit project and draft identities", () => {
  expect(editorPersistenceKey(undefined, a, "?new=1&name=Same")).toBe(`browser-${a}`);
  expect(editorPersistenceKey(undefined, b, "?new=1&name=Same")).toBe(`browser-${b}`);
  expect(editorPersistenceKey(undefined, a, "?source=local&name=Same")).toBe(`browser-${a}`);
  expect(editorPersistenceKey(undefined, b, "?source=local&name=Same")).toBe(`browser-${b}`);
  expect(editorPersistenceKey("saved-folder", a, "?draft=other")).toBe("saved-folder");
  expect(editorPersistenceKey(undefined, a, "?draft=saved-draft")).toBe("saved-draft");
});

test("same-title project runtime switches remount the captured Wizard session", () => {
  const seed = emptySeed(); seed.scene.title = "Same";
  const viewportA = `/_play/?realm=${encodeURIComponent(`https://sites.test/_project/sessions/${a}`)}`;
  const viewportB = `/_play/?realm=${encodeURIComponent(`https://sites.test/_project/sessions/${b}`)}`;
  const first = EditorWizard({ seed, viewportSrc: viewportA });
  const second = EditorWizard({ seed, viewportSrc: viewportB });
  expect(first.key).not.toBe(second.key);
  expect(EditorWizard({ seed, viewportSrc: viewportA, catalog: [] }).key).toBe(first.key);
});

test("same-name projects save and hydrate only their own virtual code, never a title-bound folder or metadata", async () => {
  const oldHandle = { name: "older-same-name-folder" } as FileSystemDirectoryHandle;
  const metadata = new Map<string, Record<string, unknown>>([["same", { codeFiles: { "src/index.ts": "old title-bound code" } }]]);
  vi.mocked(handleStore.get).mockImplementation(async key => key === "same" ? oldHandle : null);
  vi.mocked(handleStore.getMeta).mockImplementation(async key => metadata.get(key) as Awaited<ReturnType<typeof handleStore.getMeta>> ?? null);
  vi.mocked(handleStore.putMeta).mockImplementation(async (key, value) => { metadata.set(key, { ...metadata.get(key), ...value }); });
  const codeFor = (id: string, query: string): DeWorkspaceCode => {
    // an event target too: the auth context's wallet discovery listens on window
    vi.stubGlobal("window", {
      location: { href: `https://sites.test/create/scene-editor?${query}`, origin: "https://sites.test", hostname: "sites.test" },
      addEventListener: () => {}, removeEventListener: () => {}, dispatchEvent: () => true,
    });
    vi.mocked(currentProjectScope).mockReturnValue(projectScopeForId(id));
    const seed = emptySeed(); seed.scene.title = "Same";
    renderToString(<EditorWizard seed={seed} />);
    return vi.mocked(DeWorkspace).mock.calls.at(-1)![0].code!;
  };
  const codeA = codeFor(a, "new=1&name=Same");
  const codeB = codeFor(b, "source=local&name=Same");
  expect(codeA.modelNamespace).toBe(`browser-${a}`);
  expect(codeB.modelNamespace).toBe(`browser-${b}`);
  expect(codeFor(a, "new=1&name=Same").modelNamespace).toBe(codeA.modelNamespace);
  expect(await codeA.hydrate!()).toBeNull();
  expect(await codeB.hydrate!()).toBeNull();
  expect(await codeA.getDir!()).toBeNull();
  expect(await codeB.getDir!()).toBeNull();
  await codeA.persist!("src/index.ts", "project A code");
  await codeB.persist!("src/index.ts", "project B code");
  expect(await codeA.hydrate!()).toEqual({ "src/index.ts": "project A code" });
  expect(await codeB.hydrate!()).toEqual({ "src/index.ts": "project B code" });
  expect(await codeFor(a, "new=1&name=Same").hydrate!()).toEqual({ "src/index.ts": "project A code" });
  expect(handleStore.get).not.toHaveBeenCalledWith("same");
  expect(handleStore.getMeta).not.toHaveBeenCalledWith("same");
  expect(metadata.get("same")).toEqual({ codeFiles: { "src/index.ts": "old title-bound code" } });
  expect([...metadata.keys()].sort()).toEqual([`browser-${a}`, `browser-${b}`, "same"].sort());
});
