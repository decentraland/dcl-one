import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import RibbonNumeric from "./RibbonNumeric";

afterEach(cleanup);

const ROUND_TRIPPED_90 = 90.00000482967337;

describe("RibbonNumeric", () => {
  it("sends a typed value once on Enter, and blurring to Save does not send it again", () => {
    const onCommit = vi.fn();
    const view = render(<RibbonNumeric position={{ x: 0, y: 0, z: 0 }} rotation={{ x: 0, y: 0, z: 0 }} onCommit={onCommit} />);
    const rotation = screen.getByLabelText("Rotation Y");
    fireEvent.focus(rotation);
    fireEvent.change(rotation, { target: { value: "90" } });
    fireEvent.keyDown(rotation, { key: "Enter" });
    expect(onCommit).toHaveBeenCalledExactlyOnceWith("rotation", "y", 90);

    fireEvent.blur(rotation);
    view.rerender(<RibbonNumeric position={{ x: 0, y: 0, z: 0 }} rotation={{ x: 0, y: ROUND_TRIPPED_90, z: 0 }} onCommit={onCommit} />);
    fireEvent.focus(rotation);
    fireEvent.blur(rotation);
    expect(onCommit).toHaveBeenCalledOnce();
    expect(rotation).toHaveValue("90");
  });

  it("does not send the value the field already shows, and Escape drops a typed edit", () => {
    const onCommit = vi.fn();
    render(<RibbonNumeric position={{ x: 4, y: 1, z: 8 }} rotation={{ x: 0, y: ROUND_TRIPPED_90, z: 0 }} onCommit={onCommit} />);
    const rotation = screen.getByLabelText("Rotation Y");
    fireEvent.focus(rotation);
    fireEvent.change(rotation, { target: { value: "90" } });
    fireEvent.blur(rotation);

    const x = screen.getByLabelText("Position X");
    fireEvent.focus(x);
    fireEvent.change(x, { target: { value: "6" } });
    fireEvent.keyDown(x, { key: "Escape" });
    fireEvent.blur(x);
    expect(onCommit).not.toHaveBeenCalled();
    expect(x).toHaveValue("4");

    fireEvent.focus(x);
    fireEvent.change(x, { target: { value: "6" } });
    fireEvent.blur(x);
    expect(onCommit).toHaveBeenCalledExactlyOnceWith("position", "x", 6);
  });
});
