import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import DeEditorAppBar from "./DeEditorAppBar";
import appBarCss from "./deeditorappbar.css?raw";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

const options = [{ id: "scene", label: "Publish scene" }, { id: "world", label: "Publish to World" }];

describe("DeEditorAppBar", () => {
  it("prevents preview and publishing before the engine is ready and during a save", () => {
    const publish = vi.fn();
    const props = { title: "Untitled Scene", viewportSrc: "/play/?systemScene=editor", publishOptions: options, onPublish: publish };
    const { rerender } = render(<DeEditorAppBar {...props} engine="connecting" />);
    for (const engine of ["connecting", "offline"] as const) {
      rerender(<DeEditorAppBar {...props} engine={engine} />);
      for (const name of ["Preview", "Publish", "Publish options"]) {
        const button = screen.getByRole("button", { name }) as HTMLButtonElement;
        expect(button.disabled).toBe(true);
        fireEvent.click(button);
      }
    }
    expect(publish).not.toHaveBeenCalled();
    rerender(<DeEditorAppBar {...props} engine="online" />);
    fireEvent.click(screen.getByRole("button", { name: "Publish" }));
    expect(publish).toHaveBeenCalledOnce();
    rerender(<DeEditorAppBar {...props} engine="online" busy />);
    expect((screen.getByRole("button", { name: "Publish" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("preserves guarded back navigation and opens the player preview without editor parameters", () => {
    const back = vi.fn();
    const open = vi.spyOn(window, "open").mockReturnValue(null);
    render(<DeEditorAppBar title="My scene" viewportSrc="https://example.org/play/?systemScene=editor&editorUi=1&realm=project" engine="online" onExit={back} />);
    fireEvent.click(screen.getByRole("button", { name: "Back to Creator Hub" }));
    expect(back).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Preview" }));
    expect(open).toHaveBeenCalledWith("https://example.org/play/?realm=project", "_blank", "noopener,noreferrer");
    expect(screen.queryByRole("button", { name: "Preview options" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Exit" })).toBeNull();
  });

  it("renames on commit, rejects empty names, and cancels on Escape", () => {
    const rename = vi.fn();
    render(<DeEditorAppBar title="My scene" onRename={rename} />);
    const input = screen.getByRole("textbox", { name: "Scene title" }) as HTMLInputElement;
    fireEvent.change(input, { target: { value: "  Moonlit Garden  " } });
    fireEvent.blur(input);
    expect(rename).toHaveBeenCalledWith("Moonlit Garden");
    expect(input.value).toBe("My scene");
    rename.mockClear();
    fireEvent.change(input, { target: { value: " " } });
    fireEvent.blur(input);
    expect(input.value).toBe("My scene");
    fireEvent.change(input, { target: { value: "Not saved" } });
    fireEvent.keyDown(input, { key: "Escape" });
    fireEvent.blur(input);
    expect(rename).not.toHaveBeenCalled();
    expect(input.value).toBe("My scene");
  });

  it("dismisses publish options on outside click, Escape, toggle and window blur", () => {
    render(<div><p data-testid="outside">Elsewhere</p><DeEditorAppBar title="My scene" publishOptions={options} onPublish={() => {}} /></div>);
    const open = () => fireEvent.click(screen.getByLabelText("Publish options"));
    open();
    fireEvent.mouseDown(screen.getByText("Publish scene"));
    expect(screen.getByRole("menu")).toBeTruthy();
    fireEvent.mouseDown(screen.getByTestId("outside"));
    expect(screen.queryByRole("menu")).toBeNull();
    open(); fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();
    open(); open();
    expect(screen.queryByRole("menu")).toBeNull();
    open(); fireEvent.blur(window);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("on phones, Save to account and Scene settings open from More actions, which closes on a choice or Escape", () => {
    const save = vi.fn();
    const settings = vi.fn();
    render(<DeEditorAppBar title="My scene" projectTools={<nav aria-label="Project tools" className="editor-wizard__project-tools">
      <button type="button" className="editor-wizard__btn" onClick={save}>Save to account</button>
      <button type="button" className="editor-wizard__btn" onClick={settings}>Scene settings</button>
    </nav>} />);
    const more = screen.getByLabelText("More actions");
    const menu = () => more.closest(".editor-wizard__project")!;
    expect(more).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(more);
    expect(more).toHaveAttribute("aria-expanded", "true");
    expect(menu()).toHaveAttribute("data-open");
    fireEvent.click(screen.getByRole("button", { name: "Save to account" }));
    expect(save).toHaveBeenCalledOnce();
    expect(menu()).not.toHaveAttribute("data-open");
    fireEvent.click(more);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(more).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(more);
    fireEvent.click(screen.getByRole("button", { name: "Scene settings" }));
    expect(settings).toHaveBeenCalledOnce();
  });

  it("hides More actions on wide screens and, at phone width, swaps the inline project tools for it", () => {
    const style = document.createElement("style");
    style.textContent = appBarCss;
    document.head.append(style);
    const rules = (media: string | null, selector: string) => Array.from(style.sheet!.cssRules).flatMap((rule) => {
      const inner = rule instanceof CSSMediaRule ? (rule.media.mediaText.includes(media ?? "\0") ? Array.from(rule.cssRules) : []) : media ? [] : [rule];
      return inner.filter((r): r is CSSStyleRule => r instanceof CSSStyleRule && r.selectorText === selector).map((r) => r.style);
    });
    expect(rules(null, ".editor-wizard__appbar .editor-wizard__more").map((s) => s.display)).toEqual(["none"]);
    expect(rules("max-width: 600px", ".editor-wizard__appbar .editor-wizard__more").map((s) => s.display)).toEqual(["inline-flex"]);
    expect(rules("max-width: 600px", ".editor-wizard__project-tools").map((s) => s.display)).toEqual(["none"]);
    expect(rules("max-width: 600px", ".editor-wizard__project[data-open] .editor-wizard__project-tools").map((s) => [s.display, s.position])).toEqual([["flex", "absolute"]]);
    expect(rules("max-width: 600px", ".editor-wizard__appbar").map((s) => s.height)).toEqual(["124px"]);
    expect(rules("max-width: 600px", ".editor-wizard .eui-root").map((s) => s.getPropertyValue("--eui-ribbon-h").trim())).toEqual(["236px"]);
    style.remove();
  });
});
