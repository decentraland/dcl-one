import { beforeEach, expect, it, vi } from "vitest";
import { emptySeed } from "./scene-editor";
import { loadSceneEditorDraft, type SceneEditorLoaderData } from "./scene-editor-drafts";
import type { ProjectMeta } from "../../fs/handle-store";
import type { ServerDraft } from "./scene-drafts-client";

const { getMeta, fetchServerDraft } = vi.hoisted(() => ({
  getMeta: vi.fn<(slug: string) => Promise<ProjectMeta | null>>(),
  fetchServerDraft: vi.fn<() => Promise<ServerDraft | null>>(),
}));
vi.mock("../../fs/handle-store", () => ({ handleStore: { getMeta } }));
vi.mock("./scene-drafts-client", () => ({ fetchServerDraft }));

const local: ProjectMeta = { slug: "project", title: "Local edits", updatedAt: 10 };
const base: SceneEditorLoaderData = {
  sid: "test", step: null, assignment: { variant: "default" },
  seed: emptySeed("0,0"), viewportSrc: "/play/",
};
const load = () => loadSceneEditorDraft(new Request("https://sites.test/create/scene-editor?draft=project"), Promise.resolve(base));

beforeEach(() => {
  getMeta.mockReset().mockResolvedValue(local);
  fetchServerDraft.mockReset().mockResolvedValue({
    id: "project", version: 1, updatedAt: 20, title: "Server edits",
    blob: { composite: "{}", title: "Server edits" },
  });
});

it("retains loaded local edits if reading their timestamp fails", async () => {
  getMeta.mockResolvedValueOnce(local).mockRejectedValueOnce(new Error("storage unavailable"));
  expect((await load()).seed.scene.title).toBe("Local edits");
});

it.each([null, NaN, Infinity])("does not treat unknown timestamp %s as an old draft", async (timestamp) => {
  getMeta.mockResolvedValueOnce(local).mockResolvedValueOnce(timestamp === null ? null : { ...local, updatedAt: timestamp });
  expect((await load()).seed.scene.title).toBe("Local edits");
});

it.each<[number, string]>([[10, "Server edits"], [30, "Local edits"]])("compares known timestamps (%s)", async (updatedAt, title) => {
  getMeta.mockResolvedValue({ ...local, updatedAt });
  expect((await load()).seed.scene.title).toBe(title);
});

it("can recover the server draft when no local draft exists", async () => {
  getMeta.mockResolvedValue(null);
  expect((await load()).seed.scene.title).toBe("Server edits");
});
