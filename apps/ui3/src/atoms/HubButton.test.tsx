import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createRef } from "react";
import { describe, expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import HubButton from "./HubButton";
import styles from "./HubButton.module.css";

// A class of the module. A name the module does not have fails the test that asks for it.
function cls(name: string): string {
  const value = styles[name];
  if (!value) throw new Error("HubButton.module.css has no class " + name);
  return value;
}

describe("HubButton", () => {
  test("the looks of button.css keep its classes", () => {
    render(<><HubButton variant="secondary" size="sm" className="mine">Save</HubButton><HubButton variant="ghost" tone="danger">Delete</HubButton></>);

    const save = screen.getByRole("button", { name: "Save" });
    expect(save).toHaveClass("btn", "btn--secondary", "btn--sm", cls("root"), "mine");
    expect(save).toHaveAttribute("type", "button");
    expect(screen.getByRole("button", { name: "Delete" })).toHaveClass("btn", "btn--ghost", "btn--md");
    expect(screen.getByRole("button", { name: "Delete" })).toHaveAttribute("data-tone", "danger");
  });

  test("a link keeps its address and the disabled class", () => {
    render(<HubButton as="a" href="/x" aria-disabled="true">Go</HubButton>);

    const link = screen.getByRole("link", { name: "Go" });
    expect(link).toHaveClass("btn", "btn--primary", "btn--md", "is-disabled");
    expect(link).toHaveAttribute("href", "/x");
  });

  test("the text and outline looks come from the module and the neutral tone is set on the button", () => {
    render(<><HubButton variant="text" tone="neutral">Cancel</HubButton><HubButton variant="outline">More</HubButton></>);

    const cancel = screen.getByRole("button", { name: "Cancel" });
    expect(cancel).toHaveClass("btn", cls("text"), "btn--md");
    expect(cancel).not.toHaveClass("btn--primary");
    expect(cancel).toHaveAttribute("data-tone", "neutral");
    expect(screen.getByRole("button", { name: "More" })).toHaveClass("btn", cls("outline"));
  });

  test("start and end icons are hidden and stay out of the name", () => {
    render(<HubButton startIcon={<svg data-testid="s" />} endIcon={<svg data-testid="e" />}>Publish</HubButton>);

    const button = screen.getByRole("button", { name: "Publish" });
    expect(screen.getByTestId("s").parentElement).toHaveAttribute("aria-hidden", "true");
    expect(screen.getByTestId("e").parentElement).toHaveAttribute("aria-hidden", "true");
    expect(screen.getByTestId("s").parentElement).toHaveClass(cls("icon"));
    expect(button.firstElementChild).toBe(screen.getByTestId("s").parentElement);
    expect(button.lastElementChild).toBe(screen.getByTestId("e").parentElement);
  });

  test("a button with no icon holds its label only", () => {
    render(<HubButton>Plain</HubButton>);

    expect(screen.getByRole("button", { name: "Plain" }).innerHTML).toBe("Plain");
  });

  test("fullWidth adds the block class on a button and on a link", () => {
    render(<><HubButton fullWidth>Wide</HubButton><HubButton as="a" href="/x" fullWidth>Wide link</HubButton><HubButton>Narrow</HubButton></>);

    expect(screen.getByRole("button", { name: "Wide" })).toHaveClass(cls("block"));
    expect(screen.getByRole("link", { name: "Wide link" })).toHaveClass(cls("block"));
    expect(screen.getByRole("button", { name: "Narrow" })).not.toHaveClass(cls("block"));
  });

  test("a ref reaches the button and the link", () => {
    const button = createRef<HTMLButtonElement>();
    const link = createRef<HTMLAnchorElement>();
    render(<><HubButton ref={button}>One</HubButton><HubButton as="a" href="/x" ref={link}>Two</HubButton></>);

    expect(button.current).toBe(screen.getByRole("button", { name: "One" }));
    expect(link.current).toBe(screen.getByRole("link", { name: "Two" }));
  });

  test("popup attributes pass through and keys activate the button", async () => {
    const onClick = vi.fn();
    render(<HubButton aria-haspopup="menu" aria-controls="m" aria-expanded={false} onClick={onClick}>Options</HubButton>);
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

  test("a disabled button with icons is disabled and does not activate", async () => {
    const onClick = vi.fn();
    render(<HubButton variant="outline" startIcon={<svg />} disabled onClick={onClick}>Deploy</HubButton>);

    await userEvent.setup().click(screen.getByRole("button", { name: "Deploy" }));

    expect(screen.getByRole("button", { name: "Deploy" })).toBeDisabled();
    expect(onClick).not.toHaveBeenCalled();
  });

  // Button and its other users must not change: no rule of the module may match an element without a module class.
  test("every rule of the module names a class of the module", () => {
    const css = readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), "HubButton.module.css"), "utf8")
      .replace(/\/\*[\s\S]*?\*\//g, "")
      .replace(/@container[^{]*\{/g, "");
    const selectors = [...css.matchAll(/([^{}]+)\{/g)].flatMap(([, list = ""]) => list.split(",").map((s) => s.trim()));

    expect(selectors.length).toBeGreaterThan(10);
    for (const selector of selectors) {
      expect(selector.replace(/:global\([^)]*\)/g, "")).toMatch(/\.(root|block|icon|text|outline)\b/);
      expect(selector).not.toMatch(/\s/);
    }
    expect(css).not.toContain("!important");
  });
});
