import { describe, expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import Checkbox from "./Checkbox";

describe("Checkbox", () => {
  test("a call with the props ui3 uses today keeps its markup", () => {
    const { container } = render(<Checkbox>Accept</Checkbox>);

    expect(container.innerHTML).toBe(
      '<label class="checkbox"><input class="checkbox__input" type="checkbox"><span class="checkbox__box"></span><span class="checkbox__label">Accept</span></label>',
    );
  });

  test("the label names the checkbox and a click reports the new state and the event", async () => {
    const onChange = vi.fn();
    render(<Checkbox onChange={onChange}>Accept</Checkbox>);

    const box = screen.getByRole("checkbox", { name: "Accept" });
    await userEvent.setup().click(box);

    expect(box).toBeChecked();
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange.mock.calls[0]?.[0]).toBe(true);
    expect(onChange.mock.calls[0]?.[1].target).toBe(box);
  });

  test("Space toggles the focused checkbox", async () => {
    render(<Checkbox defaultChecked>Accept</Checkbox>);
    const user = userEvent.setup();

    await user.tab();
    await user.keyboard(" ");

    expect(screen.getByRole("checkbox", { name: "Accept" })).toHaveFocus();
    expect(screen.getByRole("checkbox", { name: "Accept" })).not.toBeChecked();
  });

  test("a disabled checkbox is disabled and does not change", async () => {
    const onChange = vi.fn();
    render(<Checkbox disabled onChange={onChange}>Accept</Checkbox>);

    const box = screen.getByRole("checkbox", { name: "Accept" });
    await userEvent.setup().click(box);

    expect(box).toBeDisabled();
    expect(box).not.toBeChecked();
    expect(onChange).not.toHaveBeenCalled();
  });

  test("indeterminate sets the mixed state on the input", () => {
    const { rerender } = render(<Checkbox indeterminate>All</Checkbox>);

    expect(screen.getByRole("checkbox", { name: "All" })).toBePartiallyChecked();

    rerender(<Checkbox>All</Checkbox>);
    expect(screen.getByRole("checkbox", { name: "All" })).not.toBePartiallyChecked();
  });

  test("custom icons replace the box and follow the state", async () => {
    render(<Checkbox icon={<i data-testid="off" />} checkedIcon={<i data-testid="on" />}>Pin</Checkbox>);

    expect(screen.getByTestId("off").parentElement).toHaveAttribute("aria-hidden", "true");
    await userEvent.setup().click(screen.getByRole("checkbox", { name: "Pin" }));

    expect(screen.getByTestId("on")).toBeInTheDocument();
    expect(screen.queryByTestId("off")).toBeNull();
  });

  test("ariaLabel names a checkbox with no text, and size, id, name and value reach the markup", () => {
    const { container } = render(<Checkbox ariaLabel="Select row" size="sm" id="c" name="row" value="7" className="mine" />);

    const box = screen.getByRole("checkbox", { name: "Select row" });
    expect(box).toHaveAttribute("id", "c");
    expect(box).toHaveAttribute("name", "row");
    expect(box).toHaveAttribute("value", "7");
    expect(container.firstElementChild).toHaveAttribute("data-size", "sm");
    expect(container.firstElementChild).toHaveClass("checkbox", "mine");
  });
});
