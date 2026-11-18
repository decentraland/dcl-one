import { describe, expect, test, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import Slider from "./Slider";

describe("Slider", () => {
  test("a call with the props ui3 uses today keeps its markup", () => {
    const { container } = render(<Slider label="Volume" defaultValue={30} />);

    const input = screen.getByRole("slider", { name: "Volume" });
    expect(input).toHaveValue("30");
    expect(input).not.toHaveAttribute("aria-valuetext");
    expect(input).not.toHaveAttribute("id");
    expect(container.querySelector(".slider__value")?.textContent).toBe("30");
  });

  test("a change reports the number and ariaValueText gives the spoken value", () => {
    const onChange = vi.fn();
    render(<Slider ariaLabel="Zoom" min={1} max={10} defaultValue={2} onChange={onChange} ariaValueText={(v) => v + " times"} />);

    const input = screen.getByRole("slider", { name: "Zoom" });
    expect(input).toHaveAttribute("aria-valuetext", "2 times");
    fireEvent.change(input, { target: { value: "5" } });

    expect(onChange).toHaveBeenCalledWith(5);
    expect(input).toHaveAttribute("aria-valuetext", "5 times");
  });

  test("the slider takes focus from the keyboard", async () => {
    render(<Slider ariaLabel="Zoom" />);

    await userEvent.setup().tab();

    expect(screen.getByRole("slider", { name: "Zoom" })).toHaveFocus();
  });

  test("ariaLabelledBy names the slider, id reaches the input and the value text can be left out", () => {
    const { container } = render(<><span id="l">Brightness</span><Slider ariaLabelledBy="l" id="s" showValue={false} /></>);

    expect(screen.getByRole("slider", { name: "Brightness" })).toHaveAttribute("id", "s");
    expect(container.querySelector(".slider__value")).toBeNull();
  });

  test("a disabled slider is disabled", () => {
    render(<Slider ariaLabel="Zoom" disabled />);

    expect(screen.getByRole("slider", { name: "Zoom" })).toBeDisabled();
  });
});
