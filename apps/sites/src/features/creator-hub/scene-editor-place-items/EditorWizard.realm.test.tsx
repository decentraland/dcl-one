import { renderToString } from "react-dom/server";
import { afterEach, expect, test, vi } from "vitest";
import DeWorkspace from "@ui/editor/pages/DeWorkspace";
import { emptySeed } from "@/data/lib/catalyst/creator-hub/scene-editor";
import { populateProjectRealm } from "@/data/lib/fs/project-realm";
import { stagePersistedLocalProject } from "@/data/lib/fs/local-scene";
import EditorWizard from "./EditorWizard";

vi.mock("@ui/editor/pages/DeWorkspace", () => ({ default: vi.fn(() => null) }));
vi.mock("@/data/lib/fs/project-realm", () => ({ populateProjectRealm: vi.fn(async () => ({ ok: true })), populateTemplateRealm: vi.fn(async () => ({ ok: true })) }));
vi.mock("@/data/lib/fs/local-scene", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/data/lib/fs/local-scene")>()),
  stagePersistedLocalProject: vi.fn(async () => {}),
}));
afterEach(() => { vi.clearAllMocks(); vi.unstubAllGlobals(); });

test("a blank browser project prepares its own realm instead of relying on a shared fallback", async () => {
  renderToString(<EditorWizard seed={emptySeed()} viewportSrc="/_play/?realm=%2F_project" />);
  const prepare = vi.mocked(DeWorkspace).mock.calls[0]?.[0].prepareRealm;
  expect(prepare).toBeTypeOf("function");
  vi.stubGlobal("window", { location: { href: "https://sites.test/create/scene-editor?new=1" } });
  await prepare?.();
  expect(populateProjectRealm).toHaveBeenCalledWith({}, expect.any(String), expect.objectContaining({ scope: expect.objectContaining({ cacheName: expect.stringMatching(/^ch-project-v2:/) }) }));
});

test("an account draft restores its asset map without requiring a linked physical folder", async () => {
  const assets = { "models/kept.glb": "owned-upload-cid" };
  renderToString(<EditorWizard seed={emptySeed()} projectSlug="account-draft" draftAssets={assets} viewportSrc="/_play/?realm=%2F_project" />);
  const prepare = vi.mocked(DeWorkspace).mock.calls[0]?.[0].prepareRealm;
  vi.stubGlobal("window", { location: { href: "https://sites.test/create/scene-editor?draft=account-draft&project=account-draft" } });
  await prepare?.();
  expect(stagePersistedLocalProject).not.toHaveBeenCalled();
  expect(populateProjectRealm).toHaveBeenCalledWith({}, expect.any(String), expect.objectContaining({ assets }));
});

test("a staged physical folder keeps its existing manifest during blank realm preparation", async () => {
  renderToString(<EditorWizard seed={emptySeed()} viewportSrc="/_play/?realm=%2F_project" />);
  const prepare = vi.mocked(DeWorkspace).mock.calls[0]?.[0].prepareRealm;
  vi.stubGlobal("window", { location: { href: "https://sites.test/create/scene-editor?source=local" } });
  await prepare?.();
  expect(populateProjectRealm).not.toHaveBeenCalled();
  expect(stagePersistedLocalProject).not.toHaveBeenCalled();
});

test("a saved folder opened from the scenes list, a reload or a link is staged from its folder", async () => {
  renderToString(<EditorWizard seed={emptySeed()} projectSlug="cov-a" viewportSrc="/_play/?realm=%2F_project" />);
  const prepare = vi.mocked(DeWorkspace).mock.calls[0]?.[0].prepareRealm;
  vi.stubGlobal("window", { location: { href: "https://sites.test/create/scene-editor?source=local&project=cov-a&from=scenes" } });
  await prepare?.();
  expect(stagePersistedLocalProject).toHaveBeenCalledWith("cov-a", expect.objectContaining({ cacheName: expect.stringMatching(/^ch-project-v2:/) }), undefined);
  expect(populateProjectRealm).not.toHaveBeenCalled();
});
