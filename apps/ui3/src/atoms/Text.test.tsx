import { describe, expect, test } from "vitest";
import { render, screen } from "@testing-library/react";

import Text from "./Text";

describe("Text", () => {
  test("the default is a body paragraph", () => {
    render(<Text>Hello</Text>);

    const el = screen.getByText("Hello");
    expect(el.tagName).toBe("P");
    expect(el).toHaveAttribute("data-variant", "body1");
  });

  test("a heading variant renders the heading of that level", () => {
    render(<Text variant="h5">Scenes</Text>);

    expect(screen.getByRole("heading", { level: 5, name: "Scenes" })).toHaveAttribute("data-variant", "h5");
  });

  test("`as` sets the level and the variant keeps the look", () => {
    render(<Text variant="h6" as="h2">Worlds</Text>);

    expect(screen.getByRole("heading", { level: 2, name: "Worlds" })).toHaveAttribute("data-variant", "h6");
  });

  test("a subtitle and a caption are not headings", () => {
    render(<><Text variant="subtitle1">Sub</Text><Text variant="caption">Cap</Text></>);

    expect(screen.queryByRole("heading")).toBeNull();
    expect(screen.getByText("Sub").tagName).toBe("P");
    expect(screen.getByText("Cap").tagName).toBe("SPAN");
  });

  test("tone, align, noWrap, a class and native attributes reach the element", () => {
    render(<Text tone="secondary" align="center" noWrap className="mine" id="t" title="tip">Muted</Text>);

    const el = screen.getByText("Muted");
    expect(el).toHaveAttribute("data-tone", "secondary");
    expect(el).toHaveAttribute("data-align", "center");
    expect(el).toHaveAttribute("data-nowrap");
    expect(el).toHaveClass("mine");
    expect(el).toHaveAttribute("id", "t");
    expect(el).toHaveAttribute("title", "tip");
  });

  test("the inherited tone adds no tone attribute", () => {
    render(<Text>Plain</Text>);

    expect(screen.getByText("Plain")).not.toHaveAttribute("data-tone");
  });
});
