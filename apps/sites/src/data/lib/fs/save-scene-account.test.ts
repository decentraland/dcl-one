import { afterEach, expect, it, vi } from "vitest";
import { saveSceneFromEngine } from "./save-scene";
import { pushServerDraft } from "../catalyst/creator-hub/scene-drafts-client";
import { handleStore } from "./handle-store";

vi.mock("../catalyst/creator-hub/scene-drafts-client", () => ({ pushServerDraft: vi.fn() }));
vi.mock("./handle-store", () => ({ handleStore: { putMeta: vi.fn() }, slugifyProjectTitle: (title: string) => title }));
afterEach(() => { vi.clearAllMocks(); vi.unstubAllGlobals(); });
const composite = JSON.stringify({ version: 1, components: [] });
const project = { slug: "account-scene", title: "Saved scene", base: "0,0", assets: { "kept.glb": "owned-cid" } };
const setup = () => vi.stubGlobal("window", { location: { search: "?project=account-scene" } });

it("waits for the account write before claiming persistence and caching the exported scene", async () => {
  setup();
  let confirm!: (saved: boolean) => void;
  vi.mocked(pushServerDraft).mockReturnValue(new Promise(resolve => { confirm = resolve; }));
  const writer = vi.fn();
  const pending = saveSceneFromEngine([], {}, { destination: "account", project, writer, exportComposite: async () => composite });
  await vi.waitFor(() => expect(pushServerDraft).toHaveBeenCalledOnce());
  expect(handleStore.putMeta).not.toHaveBeenCalled();
  confirm(true);
  expect(await pending).toMatchObject({ source: "engine", written: true, via: "account", serverSynced: true, text: composite });
  expect(pushServerDraft).toHaveBeenCalledWith(project.slug, { composite, title: project.title, base: project.base, assets: project.assets }, undefined);
  expect(handleStore.putMeta).toHaveBeenCalledWith(project.slug, expect.objectContaining({ composite, title: project.title, assets: project.assets }));
  expect(writer).not.toHaveBeenCalled();
});

it("a refused account write remains an error without claiming a disk or cached save", async () => {
  setup();
  vi.mocked(pushServerDraft).mockResolvedValue(false);
  const writer = vi.fn();
  await expect(saveSceneFromEngine([], {}, { destination: "account", project, writer, exportComposite: async () => composite })).rejects.toThrow("could not be saved to your account");
  expect(handleStore.putMeta).not.toHaveBeenCalled();
  expect(writer).not.toHaveBeenCalled();
});

it("a cancelled account request cannot update the browser's saved metadata", async () => {
  setup();
  const controller = new AbortController();
  vi.mocked(pushServerDraft).mockImplementation(async () => { controller.abort(); return true; });
  await expect(saveSceneFromEngine([], {}, { destination: "account", project, signal: controller.signal, exportComposite: async () => composite })).rejects.toMatchObject({ name: "AbortError" });
  expect(handleStore.putMeta).not.toHaveBeenCalled();
});
