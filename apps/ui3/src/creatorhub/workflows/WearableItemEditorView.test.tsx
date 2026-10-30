import { useState } from "react";
import { renderToString } from "react-dom/server";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { AvatarSceneOptions } from "../../wearable-preview/avatar";
import WearableItemEditorView from "./WearableItemEditorView";

const renderer = vi.hoisted(() => ({
  imported: vi.fn(),
  create: vi.fn(),
  dispose: vi.fn(),
}));

// Keep the real AvatarStage / WearablePreview lifecycle, stubbing only the 3D
// engine and the optional native-host lookup so these checks need no GPU.
vi.mock("../../explorer/components/AvatarPreview", async (original) => {
  renderer.imported();
  return original();
});
vi.mock("../../wearable-preview/bevy-host", () => ({ previewHost: async () => null }));
vi.mock("../../wearable-preview/avatar", () => ({
  createAvatarScene: (node: HTMLElement, options: AvatarSceneOptions) => {
    renderer.create(options);
    const canvas = document.createElement("canvas");
    node.append(canvas);
    options.onStatus?.("ready");
    return {
      resize() {}, setActive() {}, setOutfit() {}, setEmote() {}, setCamera() {},
      dispose() { renderer.dispose(); canvas.remove(); },
    };
  },
}));

let desktop = true;
const listeners = new Set<() => void>();

function viewport(wide: boolean) {
  act(() => {
    desktop = wide;
    listeners.forEach((listener) => listener());
  });
}

beforeEach(() => {
  desktop = true;
  renderer.create.mockClear();
  renderer.dispose.mockClear();
  vi.stubGlobal("matchMedia", () => ({
    matches: desktop,
    addEventListener: (_event: string, listener: () => void) => listeners.add(listener),
    removeEventListener: (_event: string, listener: () => void) => listeners.delete(listener),
  }));
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  listeners.clear();
});

test("SSR and item selection do not import the preview, but an explicit preview loads the renderer", async () => {
  expect(renderToString(<WearableItemEditorView value="model" step="model" />))
    .toContain("Load avatar preview");
  const view = render(<WearableItemEditorView />);
  viewport(false);
  await act(async () => {});
  expect(renderer.imported).not.toHaveBeenCalled();
  expect(renderer.create).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "Load avatar preview" })).toBeDisabled();

  viewport(true);
  fireEvent.click(screen.getByRole("button", { name: "Load avatar preview" }));
  await screen.findByRole("img", { name: "Avatar preview" });
  expect(renderer.imported).toHaveBeenCalledTimes(1);
  expect(renderer.create).toHaveBeenCalledWith(expect.objectContaining({ controls: false, platform: true }));
  view.unmount();
  expect(renderer.dispose).toHaveBeenCalledTimes(1);
});

test("starting a wearable loads its preview while preserving model selection and callbacks", async () => {
  const onSetModel = vi.fn();
  function Editor() {
    const [value, setValue] = useState("selecting");
    const [name, setName] = useState("");
    return <WearableItemEditorView
      live value={value} step={value === "selecting" ? "select" : "model"}
      draft={{ name }} onSelectNew={() => setValue("model")}
      onNameChange={setName} onSetModel={onSetModel}
    />;
  }
  render(<Editor />);
  expect(renderer.create).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "+ Add a new wearable" }));
  await screen.findByRole("img", { name: "Avatar preview" });
  fireEvent.change(screen.getByRole("textbox", { name: "Wearable name" }), { target: { value: "My hat" } });
  const model = new File(["model"], "hat.glb", { type: "model/gltf-binary" });
  fireEvent.change(screen.getByLabelText("Choose a local .glb"), { target: { files: [model] } });
  fireEvent.click(screen.getByRole("button", { name: "Set model" }));
  expect(onSetModel).toHaveBeenCalledWith("hat.glb", model);
  expect(renderer.create).toHaveBeenCalledTimes(1);
});

test("mobile can opt into an active editor without losing its draft or starting a hidden preview", async () => {
  viewport(false);
  render(<WearableItemEditorView value="model" step="model" draft={{ name: "Saved hat", modelFile: "hat.glb" }} />);
  await act(async () => {});
  expect(renderer.create).not.toHaveBeenCalled();
  // jsdom does not apply the narrow-screen CSS media rule for this notice.
  expect(screen.getByRole("link", { name: "Back to your collections", hidden: true })).toHaveAttribute("href", "/create/collections");
  fireEvent.click(screen.getByRole("button", { name: "Continue in browser", hidden: true }));
  await screen.findByRole("img", { name: "Avatar preview" });
  expect(screen.queryByRole("region", { name: "Create wearables in your browser", hidden: true })).not.toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "Wearable name" })).toHaveValue("Saved hat");
  expect(screen.getByText("hat.glb")).toBeInTheDocument();
  viewport(true);
  viewport(false);
  expect(renderer.create).toHaveBeenCalledTimes(1);
});

test("a newly covered preview stops its renderer until the mobile user continues", async () => {
  render(<WearableItemEditorView value="category" step="category" />);
  await screen.findByRole("img", { name: "Avatar preview" });
  viewport(false);
  expect(renderer.dispose).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("img", { name: "Avatar preview" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Continue in browser", hidden: true }));
  await waitFor(() => expect(renderer.create).toHaveBeenCalledTimes(2));
});

test("mobile browser entry survives a new draft while its selected model file resets", async () => {
  viewport(false);
  function Editor() {
    const [item, setItem] = useState(0);
    const [editing, setEditing] = useState(false);
    const [mobileBrowserAllowed, setMobileBrowserAllowed] = useState(false);
    return <WearableItemEditorView
      key={item} live value={editing ? "model" : "selecting"} step={editing ? "model" : "select"}
      draft={{ name: "New wearable" }} mobileBrowserAllowed={mobileBrowserAllowed}
      onContinueInBrowser={() => setMobileBrowserAllowed(true)}
      onSelectNew={() => { setItem(item + 1); setEditing(true); }}
      onBack={() => setEditing(false)}
    />;
  }
  render(<Editor />);
  fireEvent.click(screen.getByRole("button", { name: "Continue in browser", hidden: true }));
  expect(renderer.create).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "+ Add a new wearable" }));
  await screen.findByRole("img", { name: "Avatar preview" });
  expect(screen.queryByRole("region", { name: "Create wearables in your browser", hidden: true })).not.toBeInTheDocument();

  const model = new File(["model"], "hat.glb", { type: "model/gltf-binary" });
  fireEvent.change(screen.getByLabelText("Choose a local .glb"), { target: { files: [model] } });
  expect(screen.getByRole("button", { name: "Set model" })).toBeEnabled();
  fireEvent.click(screen.getByRole("button", { name: "Back" }));
  fireEvent.click(screen.getByRole("button", { name: "+ Add a new wearable" }));
  await screen.findByRole("img", { name: "Avatar preview" });
  expect(screen.queryByRole("region", { name: "Create wearables in your browser", hidden: true })).not.toBeInTheDocument();
  expect(screen.queryByText("hat.glb")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Set model" })).toBeDisabled();
});
