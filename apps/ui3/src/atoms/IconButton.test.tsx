import { createRef } from "react";
import { describe, expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import IconButton from "./IconButton";

describe("IconButton", () => {
  test("it is a button with the name from aria-label", () => {
    render(<IconButton aria-label="Close"><svg aria-hidden="true" /></IconButton>);

    const button = screen.getByRole("button", { name: "Close" });
    expect(button).toHaveAttribute("type", "button");
    expect(button).toHaveAttribute("data-size", "medium");
  });

  test("aria-labelledby gives the name too", () => {
    render(<><span id="lbl">Open menu</span><IconButton aria-labelledby="lbl">+</IconButton></>);

    expect(screen.getByRole("button", { name: "Open menu" })).toBeInTheDocument();
  });

  test("click, Enter and Space activate it", async () => {
    const onClick = vi.fn();
    render(<IconButton aria-label="Refresh" onClick={onClick}>r</IconButton>);
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "Refresh" }));
    await user.keyboard("{Enter}");
    await user.keyboard(" ");

    expect(onClick).toHaveBeenCalledTimes(3);
  });

  test("a disabled button is disabled and does not activate", async () => {
    const onClick = vi.fn();
    render(<IconButton aria-label="Delete" disabled onClick={onClick}>d</IconButton>);

    const button = screen.getByRole("button", { name: "Delete" });
    await userEvent.setup().click(button);

    expect(button).toBeDisabled();
    expect(onClick).not.toHaveBeenCalled();
  });

  test("size, edge, a class and a ref reach the button", () => {
    const ref = createRef<HTMLButtonElement>();
    render(<IconButton ref={ref} aria-label="Back" size="small" edge="start" className="mine">b</IconButton>);

    const button = screen.getByRole("button", { name: "Back" });
    expect(button).toHaveAttribute("data-size", "small");
    expect(button).toHaveAttribute("data-edge", "start");
    expect(button).toHaveClass("mine");
    expect(ref.current).toBe(button);
  });
});
