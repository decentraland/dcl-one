import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DeToolbar } from "./DeToolbar";

afterEach(cleanup);

describe("DeToolbar run controls", () => {
  it("shows one labelled Run / Stop control in the app bar's Preview language, with pause, resume and step beside it", () => {
    const onPlay = vi.fn();
    const onStop = vi.fn();
    const { rerender } = render(<DeToolbar live onPlay={onPlay} />);
    const run = screen.getByRole("button", { name: "Run the scene" });
    expect(run.className).toContain("eui-run");
    expect(run.textContent).toBe("Run");
    expect(run.querySelector("svg")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Stop the preview and return to editing" })).toBeNull();
    fireEvent.click(run);
    expect(onPlay).toHaveBeenCalledOnce();

    rerender(<DeToolbar live playing onStop={onStop} onPause={vi.fn()} />);
    const stop = screen.getByRole("button", { name: "Stop the preview and return to editing" });
    expect(stop.className).toContain("active");
    expect(stop.textContent).toBe("Stop");
    expect(screen.queryByRole("button", { name: "Run the scene" })).toBeNull();
    expect(screen.getByRole("button", { name: "Scene is running \u{2014} pause" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Advance one tick" })).toBeNull();
    fireEvent.click(stop);
    expect(onStop).toHaveBeenCalledOnce();

    rerender(<DeToolbar live paused onStop={onStop} onPlay={onPlay} onStep={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Stop the preview and return to editing" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Resume the scene" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Advance one tick" })).toBeEnabled();
  });

  it("keeps the restart control for the static toolbar", () => {
    render(<DeToolbar onStop={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Restart the scene from tick 0" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Run the scene" })).toBeDisabled();
  });
});

describe("DeToolbar camera menu", () => {
  it("closes on outside click, window blur and Escape, but stays open for clicks inside", () => {
    render(
      <div>
        <p data-testid="outside">elsewhere</p>
        <DeToolbar live onCamMode={vi.fn()} />
      </div>,
    );
    const open = () => {
      fireEvent.click(screen.getByLabelText("Camera mode"));
      expect(screen.getByText("Free fly")).toBeTruthy();
    };

    open();
    fireEvent.mouseDown(screen.getByText("Free fly"));
    expect(screen.getByText("Free fly")).toBeTruthy();
    fireEvent.mouseDown(screen.getByTestId("outside"));
    expect(screen.queryByText("Free fly")).toBeNull();

    open();
    fireEvent.blur(window);
    expect(screen.queryByText("Free fly")).toBeNull();

    open();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByText("Free fly")).toBeNull();
  });
});
