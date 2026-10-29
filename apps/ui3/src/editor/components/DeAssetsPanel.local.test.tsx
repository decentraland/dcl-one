import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { DeAssetsPanel } from "./DeAssetsPanel";
import { loadProjectModels, stashProjectModel, type CachedProjectModel } from "../project-models";
import { currentProjectScope } from "../project-cache-name";

vi.mock("../project-models", () => ({ loadProjectModels: vi.fn(), stashProjectModel: vi.fn() }));
afterEach(() => { vi.resetAllMocks(); window.history.replaceState({}, "", "/"); });
const model = (name: string): CachedProjectModel => ({ path: name, folder: "uploaded to this project", url: `${currentProjectScope().contentBase}/contents/${name}` });

test("Local reopens saved unplaced models and ignores inventory reads from the previous project", async () => {
  window.history.replaceState({}, "", "/create/scene-editor?draft=inventory-first");
  const previous = model("previous.glb");
  let finishPrevious!: (value: CachedProjectModel[]) => void;
  vi.mocked(loadProjectModels).mockImplementationOnce(() => new Promise(resolve => { finishPrevious = resolve; }));
  const view = render(<DeAssetsPanel tab="local" onPlace={() => {}} />);
  window.history.replaceState({}, "", "/create/scene-editor?draft=inventory-second");
  const saved = model("unplaced.glb");
  vi.mocked(loadProjectModels).mockResolvedValueOnce([saved]);
  view.rerender(<DeAssetsPanel tab="local" onPlace={() => {}} />);
  expect(await screen.findByTitle("Place unplaced.glb in the scene")).toBeTruthy();
  await act(async () => { finishPrevious([previous]); });
  expect(screen.queryByTitle("Place previous.glb in the scene")).toBeNull();
  expect(screen.getByTitle("Place unplaced.glb in the scene")).toBeTruthy();
});

test("a slow initial inventory read cannot remove an upload that completed meanwhile", async () => {
  let finishRead!: (value: CachedProjectModel[]) => void;
  vi.mocked(loadProjectModels).mockImplementationOnce(() => new Promise(resolve => { finishRead = resolve; }));
  const uploaded = model("new.glb");
  vi.mocked(stashProjectModel).mockResolvedValue(uploaded.url);
  const view = render(<DeAssetsPanel tab="local" onPlace={() => {}} />);
  fireEvent.change(view.container.querySelector('input[type="file"]')!, { target: { files: [new File(["model"], uploaded.path)] } });
  expect(await screen.findByTitle("Place new.glb in the scene")).toBeTruthy();
  await act(async () => { finishRead([]); });
  expect(screen.getByTitle("Place new.glb in the scene")).toBeTruthy();
  expect(vi.mocked(stashProjectModel).mock.calls[0]?.[1]?.cacheName).toBe(currentProjectScope().cacheName);
});
