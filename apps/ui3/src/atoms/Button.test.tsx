import { describe, expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import Button from "./Button";

describe("Button", () => {
  test("a button keeps its markup", () => {
    const { container } = render(<Button variant="secondary" size="sm" className="mine">Save</Button>);

    expect(container.innerHTML).toBe('<button type="button" class="btn btn--secondary btn--sm mine">Save</button>');
  });

  test("a link keeps its markup and the disabled class", () => {
    const { container } = render(<Button as="a" href="/x" aria-disabled="true">Go</Button>);

    expect(container.innerHTML).toBe('<a class="btn btn--primary btn--md is-disabled" href="/x" aria-disabled="true">Go</a>');
  });

  test("popup attributes pass through and keys activate the button", async () => {
    const onClick = vi.fn();
    render(<Button aria-haspopup="menu" aria-controls="m" aria-expanded={false} onClick={onClick}>Options</Button>);
    const user = userEvent.setup();

    const button = screen.getByRole("button", { name: "Options" });
    await user.tab();
    await user.keyboard("{Enter}");
    await user.keyboard(" ");

    expect(button).toHaveFocus();
    expect(button).toHaveAttribute("aria-haspopup", "menu");
    expect(button).toHaveAttribute("aria-controls", "m");
    expect(button).toHaveAttribute("aria-expanded", "false");
    expect(onClick).toHaveBeenCalledTimes(2);
  });

  test("a disabled button is disabled and does not activate", async () => {
    const onClick = vi.fn();
    render(<Button variant="ghost" tone="danger" disabled onClick={onClick}>Deploy</Button>);

    await userEvent.setup().click(screen.getByRole("button", { name: "Deploy" }));

    expect(screen.getByRole("button", { name: "Deploy" })).toBeDisabled();
    expect(onClick).not.toHaveBeenCalled();
  });
});
