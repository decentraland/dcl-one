import { describe, expect, test } from "vitest";
import { render, screen } from "@testing-library/react";

import * as icons from "./icons-material";
import { CloseIcon, InfoOutlinedIcon } from "./icons-material";
import stylesheet from "./icons-material.module.css?raw";

describe("material icons", () => {
  test("the file exports the 65 icons the Creator Hub uses", () => {
    expect(Object.keys(icons)).toHaveLength(65);
  });

  test("an icon is decorative by default", () => {
    const { container } = render(<CloseIcon />);

    const svg = container.querySelector("svg");
    expect(svg).toHaveAttribute("aria-hidden", "true");
    expect(svg).toHaveAttribute("focusable", "false");
    expect(screen.queryByRole("img")).toBeNull();
  });

  test("a title makes the icon an image with that name", () => {
    render(<InfoOutlinedIcon title="More information" />);

    expect(screen.getByRole("img", { name: "More information" })).not.toHaveAttribute("aria-hidden");
  });

  test("size, tone and a class of the caller reach the svg", () => {
    const { container } = render(<CloseIcon fontSize="small" tone="secondary" className="mine" data-testid="x" />);

    const svg = container.querySelector("svg");
    expect(svg).toHaveAttribute("data-size", "small");
    expect(svg).toHaveAttribute("data-tone", "secondary");
    expect(svg).toHaveClass("mine");
    expect(svg).toHaveAttribute("data-testid", "x");
  });

  test("every icon draws inside the 24 unit box", () => {
    for (const Icon of Object.values(icons)) {
      const { container, unmount } = render(<Icon />);
      const svg = container.querySelector("svg");
      expect(svg).toHaveAttribute("viewBox", "0 0 24 24");
      expect(svg?.querySelector("path, circle")).not.toBeNull();
      unmount();
    }
  });

  test("the size and tone rules weigh one class, so a caller rule with a class and an element wins", () => {
    const selectors = [...stylesheet.matchAll(/^([^\s/{}][^{}]*)\{/gm)].map((match) => (match[1] ?? "").trim());
    const sizeAndTone = selectors.filter((selector) => selector.includes("data-"));
    expect(sizeAndTone).toHaveLength(8);
    for (const selector of sizeAndTone) expect(selector).toMatch(/^\.root:where\(\[data-(size|tone)="[a-z]+"\]\)$/);
  });
});
