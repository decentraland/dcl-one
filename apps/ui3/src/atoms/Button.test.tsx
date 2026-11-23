import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createRef } from "react";
import { describe, expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import Button from "./Button";
import styles from "./Button.module.css";

// A class of the module. A name the module does not have fails the test that asks for it.
function cls(name: string): string {
  const value = styles[name];
  if (!value) throw new Error("Button.module.css has no class " + name);
  return value;
}

describe("Button", () => {
  test("a button keeps its markup", () => {
    const { container } = render(<Button variant="secondary" size="sm" className="mine">Save</Button>);

    expect(container.innerHTML).toBe('<button type="button" class="btn btn--secondary btn--sm mine">Save</button>');
  });

  test("a link keeps its markup and the disabled class", () => {
    const { container } = render(<Button as="a" href="/x" aria-disabled="true">Go</Button>);

    expect(container.innerHTML).toBe('<a class="btn btn--primary btn--md is-disabled" href="/x" aria-disabled="true">Go</a>');
  });

  test("a tone is set on the button", () => {
    render(<Button variant="ghost" tone="danger">Delete</Button>);

    const button = screen.getByRole("button", { name: "Delete" });
    expect(button).toHaveClass("btn", "btn--ghost", "btn--md");
    expect(button).toHaveAttribute("data-tone", "danger");
  });

  test("the text and outline looks and the neutral tone come from the module", () => {
    render(<><Button variant="text" tone="neutral">Cancel</Button><Button variant="outline">More</Button></>);

    const cancel = screen.getByRole("button", { name: "Cancel" });
    expect(cancel).toHaveClass("btn", cls("text"), cls("neutral"), "btn--md");
    expect(cancel).not.toHaveClass("btn--primary");
    expect(cancel).toHaveAttribute("data-tone", "neutral");
    expect(screen.getByRole("button", { name: "More" })).toHaveClass("btn", cls("outline"));
  });

  test("start and end icons are hidden and stay out of the name", () => {
    render(<Button startIcon={<svg data-testid="s" />} endIcon={<svg data-testid="e" />}>Publish</Button>);

    const button = screen.getByRole("button", { name: "Publish" });
    expect(screen.getByTestId("s").parentElement).toHaveAttribute("aria-hidden", "true");
    expect(screen.getByTestId("e").parentElement).toHaveAttribute("aria-hidden", "true");
    expect(screen.getByTestId("s").parentElement).toHaveClass(cls("icon"));
    expect(button.firstElementChild).toBe(screen.getByTestId("s").parentElement);
    expect(button.lastElementChild).toBe(screen.getByTestId("e").parentElement);
  });

  test("fullWidth adds the block class on a button and on a link", () => {
    render(<><Button fullWidth>Wide</Button><Button as="a" href="/x" fullWidth>Wide link</Button><Button>Narrow</Button></>);

    expect(screen.getByRole("button", { name: "Wide" })).toHaveClass(cls("block"));
    expect(screen.getByRole("link", { name: "Wide link" })).toHaveClass(cls("block"));
    expect(screen.getByRole("button", { name: "Narrow" })).not.toHaveClass(cls("block"));
  });

  test("a ref reaches the button and the link", () => {
    const button = createRef<HTMLButtonElement>();
    const link = createRef<HTMLAnchorElement>();
    render(<><Button ref={button}>One</Button><Button as="a" href="/x" ref={link}>Two</Button></>);

    expect(button.current).toBe(screen.getByRole("button", { name: "One" }));
    expect(link.current).toBe(screen.getByRole("link", { name: "Two" }));
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
    render(<Button variant="outline" tone="danger" startIcon={<svg />} disabled onClick={onClick}>Deploy</Button>);

    await userEvent.setup().click(screen.getByRole("button", { name: "Deploy" }));

    expect(screen.getByRole("button", { name: "Deploy" })).toBeDisabled();
    expect(onClick).not.toHaveBeenCalled();
  });

  // The other users of Button must not change: no rule of the module may match an element without a module class.
  test("every rule of the module names a class of the module", () => {
    const css = readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), "Button.module.css"), "utf8")
      .replace(/\/\*[\s\S]*?\*\//g, "");
    const selectors = [...css.matchAll(/([^{}]+)\{/g)].flatMap(([, list = ""]) => list.split(",").map((s) => s.trim()));

    expect(selectors.length).toBeGreaterThan(8);
    for (const selector of selectors) {
      expect(selector.replace(/:global\([^)]*\)/g, "")).toMatch(/\.(block|icon|text|outline|neutral)\b/);
      expect(selector).not.toMatch(/\s/);
    }
    expect(css).not.toContain("!important");
  });
});
