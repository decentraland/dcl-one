import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";

import { ADD_COMPONENTS, DeInspectorPanel, filterAddComponents } from "./DeInspectorPanel";

afterEach(cleanup);

describe("DeInspectorPanel inline fields", () => {
  it("shows the model path as the scene names it, and explains a model that never loaded", () => {
    const loaded = render(
      <DeInspectorPanel
        name="Wooden Door"
        id="512"
        live
        components={["Transform", "GltfContainer", "GltfContainerLoadingState"]}
        componentValues={{
          GltfContainer: { src: "assets/imported/door/door.glb" },
          GltfContainerLoadingState: { currentState: 4 },
        }}
        onAuthorComponent={() => {}}
      />,
    );
    const body = within(screen.getByRole("region", { name: "Entity components" }));
    expect((body.getByLabelText("src") as HTMLInputElement).value).toBe(
      "assets/imported/door/door.glb",
    );
    expect(body.getByText("Loaded")).toBeTruthy();
    expect(body.queryByText(/No inline fields/)).toBeNull();
    loaded.unmount();

    render(
      <DeInspectorPanel
        name="Door"
        id="513"
        live
        components={["GltfContainerLoadingState"]}
        componentValues={{ GltfContainerLoadingState: { currentState: 2 } }}
      />,
    );
    expect(screen.getByText(/model file is missing/)).toBeTruthy();
  });
});


describe("DeInspectorPanel component picker", () => {
  it("closes after a ribbon reveal and reopens only for a new reveal", () => {
    const authored: string[] = [];
    const props: ComponentProps<typeof DeInspectorPanel> = {
      name: "Model", id: "512", addOpen: true,
      onAuthorComponent: (_id, name) => { authored.push(name); },
    };
    const view = render(<DeInspectorPanel {...props} revealNonce={1} />);
    fireEvent.click(screen.getByTitle("Billboard"));
    expect(authored).toEqual(["Billboard"]);
    expect(screen.queryByTitle("Billboard")).toBeNull();
    expect(screen.getByRole("button", { name: "Add component" }).getAttribute("aria-expanded")).toBe("false");
    view.rerender(<DeInspectorPanel {...props} revealNonce={1} />);
    expect(screen.queryByTitle("Billboard")).toBeNull();
    view.rerender(<DeInspectorPanel {...props} revealNonce={2} />);
    expect(screen.getByTitle("Billboard")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Add component" }));
    expect(screen.queryByTitle("Billboard")).toBeNull();
  });
});

describe("inspector authoring permissions", () => {
  it.each(["GltfContainerLoadingState", "core::GltfContainerLoadingState"])("keeps %s inspectable but prevents edits and deletion", name => {
    const write = vi.fn(), remove = vi.fn(), copy = vi.fn(), paste = vi.fn();
    render(<DeInspectorPanel id="512" live components={[name, "AudioSource"]}
      writableComponents={new Set(["AudioSource"])}
      componentValues={{ [name]: { currentState: 3 }, AudioSource: { audioClipUrl: "tone.wav" } }}
      onAuthorComponent={write} onDeleteComponent={remove} clipboard={{ copy, paste }} />);
    const loading = screen.getByText("Gltf Container Loading State").closest(".eui-comp")! as HTMLElement;
    expect(within(loading).getByRole("button", { name: "Remove component" })).toBeDisabled();
    expect(within(loading).queryByRole("button", { name: "Edit as JSON" })).toBeNull();
    expect(within(loading).queryByRole("button", { name: /paste/i })).toBeNull();
    expect(loading.textContent).toMatch(/Model/);
    const audio = screen.getByText("Audio Source").closest(".eui-comp")! as HTMLElement;
    fireEvent.click(within(audio).getByRole("button", { name: "Remove component" }));
    expect(remove).toHaveBeenCalledExactlyOnceWith("512", "AudioSource");
    expect(write).not.toHaveBeenCalled();
  });
});

describe("DeInspectorPanel header", () => {
  it("keeps the entity name primary and demotes Make interactive to an icon with a tooltip", () => {
    render(<DeInspectorPanel name="Escape Room" id="0" onAuthorComponent={() => {}} />);
    const head = screen.getByLabelText("Entity name").closest(".eui-panel-head")! as HTMLElement;
    const wire = within(head).getByRole("button", { name: "Add interaction" });
    expect(wire.textContent).toBe("");
    expect(wire.className).toContain("icon");
    expect(wire.title).toMatch(/^Make interactive/);
    // Add component sits at the top of the body, so the head keeps the name, the id and the bolt only.
    expect(within(head).queryByRole("button", { name: "Add component" })).toBeNull();
    const add = within(screen.getByRole("region", { name: "Entity components" })).getByRole("button", { name: "Add component" });
    expect(add.textContent).toBe("Add component");
    expect(within(head).getAllByRole("button")).toHaveLength(1);
    fireEvent.click(wire);
    expect(wire.getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByText(/Make .Escape Room. interactive/)).toBeTruthy();
  });
});

describe("DeInspectorPanel add-component menu", () => {
  it("shows every entry with its label, technical name and description, grouped, and filters by search", () => {
    render(<DeInspectorPanel name="Model" id="512" addOpen onAuthorComponent={() => {}} />);
    const menu = within(screen.getByRole("group", { name: "Add component" }));
    expect(menu.getAllByRole("button")).toHaveLength(ADD_COMPONENTS.length);
    expect(menu.getByText("3D Content")).toBeTruthy();
    expect(menu.getByText("Smart item")).toBeTruthy();
    const model = menu.getByTitle("GltfContainer");
    expect(model.textContent).toContain("3D model");
    expect(model.textContent).toContain("GltfContainer");
    expect(model.textContent).toContain("Show a .glb / .gltf model");
    const search = menu.getByLabelText("Search components");
    fireEvent.change(search, { target: { value: "sound" } });
    expect(menu.getAllByRole("button").map(b => b.title)).toEqual(["AudioSource", "asset-packs::Actions"]);
    expect(menu.queryByText("3D Content")).toBeNull();
    fireEvent.change(search, { target: { value: "zzz" } });
    expect(menu.getByText(/No component matches/)).toBeTruthy();
  });

  it("adds the first match on Enter in the search field", () => {
    const authored: string[] = [];
    render(<DeInspectorPanel name="Model" id="512" addOpen onAuthorComponent={(_id, name) => { authored.push(name); }} />);
    const search = screen.getByLabelText("Search components");
    fireEvent.change(search, { target: { value: "trigger" } });
    fireEvent.keyDown(search, { key: "Enter" });
    expect(authored).toEqual(["asset-packs::Triggers"]);
  });

  it("matches on label, technical name and description, label and name matches first", () => {
    expect(filterAddComponents("face the player").map(c => c.name)).toEqual(["Billboard"]);
    expect(filterAddComponents("trigger").map(c => c.name)).toEqual(["asset-packs::Triggers", "asset-packs::States"]);
    expect(filterAddComponents("nftshape").map(c => c.name)).toEqual(["NftShape"]);
    expect(filterAddComponents("every player").map(c => c.name)).toEqual(["core-schema::Sync-Components"]);
    expect(filterAddComponents("  ")).toHaveLength(ADD_COMPONENTS.length);
  });
});

describe("DeInspectorPanel model import", () => {
  const gltf = (src: string) => ({
    name: "Door", id: "512", live: true,
    components: ["GltfContainer"], componentValues: { GltfContainer: { src } },
  });

  it("opens a glb/gltf file picker from the folder button, imports the file, and writes the returned path as src", async () => {
    const onImportModel = vi.fn(async (file: File) => `assets/imported/local-${file.name}/${file.name}`);
    const write = vi.fn();
    render(<DeInspectorPanel {...gltf("old.glb")} onAuthorComponent={write} onImportModel={onImportModel} />);
    const picker = screen.getByLabelText("Model file") as HTMLInputElement;
    expect(picker.type).toBe("file");
    expect(picker.accept).toBe(".glb,.gltf,model/gltf-binary,model/gltf+json");
    const folder = screen.getByRole("button", { name: "Import model file" });
    expect(folder).toBeEnabled();
    const click = vi.spyOn(picker, "click");
    fireEvent.click(folder);
    expect(click).toHaveBeenCalledOnce();
    const file = new File(["glb"], "door.glb", { type: "model/gltf-binary" });
    fireEvent.change(picker, { target: { files: [file] } });
    await waitFor(() => expect(write).toHaveBeenCalledOnce());
    expect(onImportModel).toHaveBeenCalledExactlyOnceWith(file);
    expect(write).toHaveBeenCalledWith("512", "GltfContainer", JSON.stringify({ src: "assets/imported/local-door.glb/door.glb" }));
    expect(screen.getByRole("status").textContent).toBe("Imported door.glb");
  });

  it("reports an import failure without touching src, and disables the button when the scene cannot be edited", async () => {
    const write = vi.fn();
    render(<DeInspectorPanel {...gltf("old.glb")} onAuthorComponent={write} onImportModel={async () => { throw new Error("assets/x already contains a different asset."); }} />);
    const file = new File(["glb"], "door.glb");
    fireEvent.change(screen.getByLabelText("Model file"), { target: { files: [file] } });
    await waitFor(() => expect(screen.getByRole("status").textContent).toMatch(/already contains a different asset/));
    expect(write).not.toHaveBeenCalled();
    cleanup();
    render(<DeInspectorPanel {...gltf("old.glb")} onAuthorComponent={write} />);
    expect(screen.getByRole("button", { name: "Import model file" })).toBeDisabled();
  });

  it("offers the project's existing models as src suggestions", async () => {
    const assets = {
      list: async () => [{ path: "assets/imported/lamp/lamp.glb", size: 1 }, { path: "images/a.png", size: 1 }, { path: "models/tree.gltf", size: 1 }],
      read: async () => { throw new Error("unused"); }, write: async () => {}, remove: async () => {},
    };
    render(<DeInspectorPanel {...gltf("old.glb")} onAuthorComponent={() => {}} assets={assets} />);
    const src = screen.getByLabelText("src") as HTMLInputElement;
    await waitFor(() => expect(src.getAttribute("list")).toBeTruthy());
    const options = Array.from(document.getElementById(src.getAttribute("list")!)!.querySelectorAll("option")).map(o => o.value);
    expect(options).toEqual(["assets/imported/lamp/lamp.glb", "models/tree.gltf"]);
  });
});
