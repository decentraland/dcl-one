import { describe, expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import Chip from "./Chip";

describe("Chip", () => {
  test("a chip with no handler is static text", () => {
    render(<Chip label="Beta" />);

    expect(screen.queryByRole("button")).toBeNull();
    const chip = screen.getByText("Beta").parentElement;
    expect(chip?.tagName).toBe("SPAN");
    expect(chip).toHaveAttribute("data-variant", "filled");
    expect(chip).toHaveAttribute("data-size", "medium");
  });

  test("a chip with a handler is a button that click and keys activate", async () => {
    const onClick = vi.fn();
    render(<Chip label="Filter" variant="outlined" size="small" onClick={onClick} />);
    const user = userEvent.setup();

    const button = screen.getByRole("button", { name: "Filter" });
    await user.click(button);
    await user.keyboard("{Enter}");

    expect(button).toHaveAttribute("data-variant", "outlined");
    expect(button).toHaveAttribute("data-size", "small");
    expect(onClick).toHaveBeenCalledTimes(2);
  });

  test("a disabled chip does not activate", async () => {
    const onClick = vi.fn();
    render(<Chip label="Filter" onClick={onClick} disabled />);

    await userEvent.setup().click(screen.getByRole("button", { name: "Filter" }));

    expect(screen.getByRole("button", { name: "Filter" })).toBeDisabled();
    expect(onClick).not.toHaveBeenCalled();
  });

  test("the icon is hidden from the name and the pressed state is exposed", () => {
    render(<Chip label="Pinned" icon={<svg data-testid="pin" />} onClick={() => {}} aria-pressed />);

    expect(screen.getByRole("button", { name: "Pinned", pressed: true })).toBeInTheDocument();
    expect(screen.getByTestId("pin").parentElement).toHaveAttribute("aria-hidden", "true");
  });
});
