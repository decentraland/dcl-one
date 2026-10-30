import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import DeCodeWorkspace from "./DeCodeWorkspace";
import * as folders from "../folder-project";

const fixture = vi.hoisted(() => ({ models: new Map<string, any>(), current: null as any, changed: () => {} }));
vi.mock("./monaco-host", () => {
  const modelFor = (_: unknown, path: string, content: string) => {
    const key = `file:///${path}`;
    if (!fixture.models.has(key)) {
      let value = content;
      fixture.models.set(key, { getValue: () => value, setValue: (next: string) => { value = next; fixture.changed(); } });
    }
    return fixture.models.get(key);
  };
  return { modelFor, languageFor: () => "typescript", loadMonaco: async () => ({ typesLoaded: 1, monaco: {
    Uri: { parse: (value: string) => value }, KeyMod: { CtrlCmd: 1 }, KeyCode: { KeyS: 2 },
    editor: { getModel: (uri: string) => fixture.models.get(uri), create: () => ({
      addCommand: () => {}, dispose: () => {}, onDidChangeModelContent: (fn: () => void) => { fixture.changed = fn; },
      setModel: (model: unknown) => { fixture.current = model; fixture.changed(); },
    }) },
  } }) };
});

beforeEach(() => { fixture.models.clear(); fixture.current = null; });
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

it("routes a physical folder through a private revision session and retains edits on conflict", async () => {
  const session = { id: "folder", list: async () => ["src/index.ts"], read: vi.fn(async () => "folder original"), write: vi.fn().mockRejectedValue(new Error("File changed outside the editor")) };
  const folder = vi.spyOn(folders, "folderProject").mockReturnValue(session);
  const directory = { name: "scene", async *values() { yield { name: "src", kind: "directory", async *values() { yield { name: "index.ts", kind: "file" }; } }; } } as unknown as FileSystemDirectoryHandle;
  render(<DeCodeWorkspace code={{ getDir: async () => directory }} />);
  await waitFor(() => expect(fixture.current?.getValue()).toBe("folder original"));
  expect(folder).toHaveBeenCalledWith(directory);
  await act(async () => { fixture.current.setValue("folder edit"); });
  await waitFor(() => expect(document.querySelector(".decode-dirty")).not.toBeNull());
  fireEvent.click(screen.getByRole("button", { name: /index.ts/ }));
  expect(session.read).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("button", { name: /^Save$/ }));
  expect(await screen.findByRole("alert")).toHaveTextContent("File changed outside");
  expect(session.write).toHaveBeenCalledWith("src/index.ts", "folder edit");
  expect(fixture.current.getValue()).toBe("folder edit");
  expect(document.querySelector(".decode-dirty")).not.toBeNull();
  expect([...fixture.models.keys()][0]).toMatch(/^file:\/\/\/folder\//);
});

it("keeps one private file session through Monaco rerenders", async () => {
  const session = { id: "sdk", list: async () => ["src/index.ts"], read: vi.fn(async () => "original"), write: vi.fn().mockResolvedValue(undefined) };
  const shared = { ...session, read: vi.fn(async () => "wrong session"), createFileSession: vi.fn(() => session) };
  const { rerender } = render(<DeCodeWorkspace code={{ project: shared }} />);
  await waitFor(() => expect(fixture.current?.getValue()).toBe("original"));
  rerender(<DeCodeWorkspace code={{ project: shared }} />);
  expect(shared.createFileSession).toHaveBeenCalledTimes(1);
  expect(shared.read).not.toHaveBeenCalled();
  await act(async () => { fixture.current.setValue("edited"); });
  await waitFor(() => expect(document.querySelector(".decode-dirty")).not.toBeNull());
  fireEvent.click(screen.getByRole("button", { name: /^Save$/ }));
  await waitFor(() => expect(session.write).toHaveBeenCalledWith("src/index.ts", "edited"));
});

it("uses the SDK project files and preserves dirty edits across file switches", async () => {
  const project = { id: "http://localhost:8000", list: async () => ["src/index.ts", "scene.json"],
    read: vi.fn(async (path: string) => path === "scene.json" ? "{}" : "original"), write: vi.fn().mockResolvedValue(undefined) };
  render(<DeCodeWorkspace code={{ project }} />);
  await waitFor(() => expect(fixture.current?.getValue()).toBe("original"));
  fireEvent.click(screen.getByRole("button", { name: "index.ts" }));
  await waitFor(() => expect(project.read).toHaveBeenCalledTimes(2));
  await act(async () => { fixture.current.setValue("edited"); });
  await waitFor(() => expect(document.querySelector(".decode-dirty")).not.toBeNull());
  fireEvent.click(screen.getByRole("button", { name: "scene.json" }));
  await waitFor(() => expect(fixture.current.getValue()).toBe("{}"));
  fireEvent.click(screen.getByRole("button", { name: /index.ts/ }));
  await waitFor(() => expect(fixture.current.getValue()).toBe("edited"));
  expect(project.read.mock.calls.filter(([path]) => path === "src/index.ts")).toHaveLength(2);
  fireEvent.click(screen.getByRole("button", { name: /^Save$/ }));
  await waitFor(() => expect(project.write).toHaveBeenCalledWith("src/index.ts", "edited"));
});

it("does not clear unsaved edits or claim success when the SDK rejects a conflicting save", async () => {
  const project = { id: "http://localhost:8000", list: async () => ["src/index.ts"], read: async () => "original",
    write: vi.fn().mockRejectedValue(new Error("File changed outside the editor")) };
  render(<DeCodeWorkspace code={{ project }} />);
  await waitFor(() => expect(fixture.current?.getValue()).toBe("original"));
  await act(async () => { fixture.current.setValue("edited"); });
  await waitFor(() => expect(document.querySelector(".decode-dirty")).not.toBeNull());
  fireEvent.click(screen.getByRole("button", { name: /^Save$/ }));
  expect(await screen.findByRole("alert")).toHaveTextContent("File changed outside");
  expect(document.querySelector(".decode-dirty")).not.toBeNull();
});

it("disables editing while a file opens and ignores an older response after switching again", async () => {
  let resolveScene: (value: string) => void = () => {};
  const scene = new Promise<string>((resolve) => { resolveScene = resolve; });
  const project = { id: "http://localhost:8000", list: async () => ["src/index.ts", "scene.json"],
    read: async (path: string) => path === "scene.json" ? scene : "source", write: vi.fn() };
  render(<DeCodeWorkspace code={{ project }} />);
  await waitFor(() => expect(fixture.current?.getValue()).toBe("source"));
  fireEvent.click(screen.getByRole("button", { name: "scene.json" }));
  expect(fixture.current).toBeNull();
  expect(screen.getByRole("button", { name: /^Save$/ })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "index.ts" }));
  await waitFor(() => expect(fixture.current?.getValue()).toBe("source"));
  await act(async () => { resolveScene("{}"); await scene; });
  expect(fixture.current.getValue()).toBe("source");
  expect(screen.queryByRole("status")).toBeNull();
});

it("isolates same-path virtual models and saved baselines when a mounted workspace changes drafts", async () => {
  const store = new Map<string, string>();
  const codeA = { modelNamespace: "draft A", virtualFiles: [{ path: "src/index.ts", text: "A saved" }], persist: vi.fn(async () => {}) };
  const codeB = { modelNamespace: "draft B", virtualFiles: [{ path: "src/index.ts", text: "B saved" }], persist: vi.fn(async () => {}) };
  const { rerender } = render(<DeCodeWorkspace code={codeA} store={store} />);
  await waitFor(() => expect(fixture.current?.getValue()).toBe("A saved"));
  const modelA = fixture.current;
  await act(async () => { modelA.setValue("A unsaved"); });
  rerender(<DeCodeWorkspace code={codeB} store={store} />);
  await waitFor(() => expect(fixture.current?.getValue()).toBe("B saved"));
  expect(fixture.current).not.toBe(modelA);
  expect(document.querySelector(".decode-dirty")).toBeNull();
  expect([...fixture.models.keys()]).toEqual(["file:///browser/64726166742041/src/index.ts", "file:///browser/64726166742042/src/index.ts"]);
  rerender(<DeCodeWorkspace code={codeA} store={store} />);
  await waitFor(() => expect(fixture.current).toBe(modelA));
  expect(fixture.current.getValue()).toBe("A unsaved");
  expect(document.querySelector(".decode-dirty")).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Save (draft)" }));
  await waitFor(() => expect(codeA.persist).toHaveBeenCalledWith("src/index.ts", "A unsaved"));
  expect(codeB.persist).not.toHaveBeenCalled();
});

it("reopening a draft retains unsaved virtual content and dirty state, including after a failed save", async () => {
  const store = new Map<string, string>();
  const code = { modelNamespace: "reopen", virtualFiles: [{ path: "src/index.ts", text: "saved" }], persist: vi.fn().mockRejectedValue(new Error("quota")) };
  const first = render(<DeCodeWorkspace code={code} store={store} />);
  await waitFor(() => expect(fixture.current?.getValue()).toBe("saved"));
  const model = fixture.current;
  await act(async () => { model.setValue("unsaved"); });
  fireEvent.click(screen.getByRole("button", { name: "Save (draft)" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("quota");
  expect(store.get("src/index.ts")).toBe("saved");
  first.unmount();
  render(<DeCodeWorkspace code={code} store={store} />);
  await waitFor(() => expect(screen.queryByRole("status")).toBeNull());
  expect(fixture.current).toBe(model);
  expect(model.getValue()).toBe("unsaved");
  expect(document.querySelector(".decode-dirty")).not.toBeNull();
});

it("preserves unsaved virtual file tabs without adopting another file's saved baseline", async () => {
  render(<DeCodeWorkspace code={{ modelNamespace: "file-tabs", virtualFiles: [{ path: "src/index.ts", text: "source" }, { path: "scene.json", text: "{}" }] }} />);
  await waitFor(() => expect(fixture.current?.getValue()).toBe("source"));
  await act(async () => { fixture.current.setValue("edited source"); });
  fireEvent.click(screen.getByRole("button", { name: "scene.json" }));
  await waitFor(() => expect(fixture.current.getValue()).toBe("{}"));
  fireEvent.click(screen.getByRole("button", { name: /index.ts/ }));
  await waitFor(() => expect(fixture.current.getValue()).toBe("edited source"));
  expect(document.querySelector(".decode-dirty")).not.toBeNull();
});

it("does not reuse anonymous virtual models across unrelated unscoped workspaces", async () => {
  const first = render(<DeCodeWorkspace code={{ virtualFiles: [{ path: "src/index.ts", text: "first" }] }} />);
  await waitFor(() => expect(fixture.current?.getValue()).toBe("first"));
  first.unmount();
  render(<DeCodeWorkspace code={{ virtualFiles: [{ path: "src/index.ts", text: "second" }] }} />);
  await waitFor(() => expect(fixture.current?.getValue()).toBe("second"));
  expect(fixture.models.size).toBe(2);
});

it("retires a pending virtual hydrate when switching to a different draft", async () => {
  let finish!: (files: Record<string, string>) => void;
  const pending = new Promise<Record<string, string>>(resolve => { finish = resolve; });
  const hydrate = vi.fn(() => pending);
  const { rerender } = render(<DeCodeWorkspace code={{ modelNamespace: "old", virtualFiles: [{ path: "src/index.ts", text: "old" }], hydrate }} />);
  await waitFor(() => expect(hydrate).toHaveBeenCalledOnce());
  rerender(<DeCodeWorkspace code={{ modelNamespace: "new", virtualFiles: [{ path: "src/index.ts", text: "new" }] }} />);
  await waitFor(() => expect(fixture.current?.getValue()).toBe("new"));
  await act(async () => { finish({ "src/index.ts": "late old response" }); await pending; });
  expect(fixture.current.getValue()).toBe("new");
  expect(fixture.models.size).toBe(1);
});

it("deliberate native Reload refreshes the existing disk model only after dirty-edit confirmation", async () => {
  let physical = "folder original";
  const session = { id: "folder", list: async () => ["src/index.ts"], read: vi.fn(async () => physical), write: vi.fn() };
  vi.spyOn(folders, "folderProject").mockReturnValue(session);
  const directory = { name: "scene", async *values() { yield { name: "src", kind: "directory", async *values() { yield { name: "index.ts", kind: "file" }; } }; } } as unknown as FileSystemDirectoryHandle;
  render(<DeCodeWorkspace code={{ getDir: async () => directory }} />);
  await waitFor(() => expect(fixture.current?.getValue()).toBe("folder original"));
  const model = fixture.current;
  await act(async () => { model.setValue("local edit"); });
  physical = "external disk edit";
  fireEvent.click(screen.getByRole("button", { name: /index.ts/ }));
  expect(session.read).toHaveBeenCalledOnce();
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
  fireEvent.click(screen.getByRole("button", { name: "Reload file" }));
  expect(session.read).toHaveBeenCalledOnce();
  expect(model.getValue()).toBe("local edit");
  confirm.mockReturnValue(true);
  fireEvent.click(screen.getByRole("button", { name: "Reload file" }));
  await waitFor(() => expect(model.getValue()).toBe("external disk edit"));
  expect(fixture.current).toBe(model);
  expect(document.querySelector(".decode-dirty")).toBeNull();
});
