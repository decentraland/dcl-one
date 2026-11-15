import { createRef, useState } from "react";
import { describe, expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import TextField from "./TextField";

describe("TextField", () => {
  test("the label names the input", () => {
    render(<TextField label="Scene name" defaultValue="Plaza" />);

    expect(screen.getByRole("textbox", { name: "Scene name" })).toHaveValue("Plaza");
  });

  test("typing calls onChange with the native event and a controlled value follows", async () => {
    const seen: string[] = [];
    function Field() {
      const [value, setValue] = useState("");
      return <TextField aria-label="Title" value={value} onChange={(e) => { seen.push(e.target.value); setValue(e.target.value); }} />;
    }
    render(<Field />);

    await userEvent.setup().type(screen.getByRole("textbox", { name: "Title" }), "ab");

    expect(seen).toEqual(["a", "ab"]);
    expect(screen.getByRole("textbox", { name: "Title" })).toHaveValue("ab");
  });

  test("an error marks the input invalid and the helper text describes it", () => {
    render(<TextField label="Name" error helperText="The name is taken" />);

    const input = screen.getByRole("textbox", { name: "Name" });
    expect(input).toBeInvalid();
    expect(input).toHaveAccessibleDescription("The name is taken");
  });

  test("a field with no error is not marked invalid", () => {
    render(<TextField label="Name" />);

    expect(screen.getByRole("textbox", { name: "Name" })).not.toHaveAttribute("aria-invalid");
  });

  test("a disabled field is disabled and takes no text", async () => {
    const onChange = vi.fn();
    render(<TextField label="Name" disabled onChange={onChange} />);

    const input = screen.getByRole("textbox", { name: "Name" });
    await userEvent.setup().type(input, "x");

    expect(input).toBeDisabled();
    expect(onChange).not.toHaveBeenCalled();
  });

  test("adornments render beside the input and stay out of its name", () => {
    render(<TextField label="Amount" startAdornment={<span>$</span>} endAdornment={<button type="button">Max</button>} />);

    expect(screen.getByRole("textbox", { name: "Amount" })).toBeInTheDocument();
    expect(screen.getByText("$")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Max" })).toBeInTheDocument();
  });

  test("multiline renders a textarea with the minimum rows and the row limits", () => {
    render(<TextField label="Description" multiline minRows={3} maxRows={6} />);

    const area = screen.getByRole("textbox", { name: "Description" });
    expect(area.tagName).toBe("TEXTAREA");
    expect(area).toHaveAttribute("rows", "3");
    expect(area.style.getPropertyValue("--field-min-rows")).toBe("3");
    expect(area.style.getPropertyValue("--field-max-rows")).toBe("6");
  });

  test("key and blur handlers, the placeholder, the type and the ref reach the input", async () => {
    const onKeyDown = vi.fn();
    const onBlur = vi.fn();
    const ref = createRef<HTMLInputElement>();
    render(<TextField aria-label="Search" placeholder="Find" type="search" inputRef={ref} onKeyDown={onKeyDown} onBlur={onBlur} />);
    const user = userEvent.setup();

    const input = screen.getByPlaceholderText("Find");
    await user.type(input, "{Enter}");
    expect(onKeyDown).toHaveBeenCalledTimes(1);
    await user.tab();

    expect(input).toHaveAttribute("type", "search");
    expect(ref.current).toBe(input);
    expect(onBlur).toHaveBeenCalledTimes(1);
  });

  test("size, tone and full width are set on the root", () => {
    const { container } = render(<TextField aria-label="Name" size="small" tone="neutral" fullWidth />);

    const root = container.firstElementChild;
    expect(root).toHaveAttribute("data-size", "small");
    expect(root).toHaveAttribute("data-tone", "neutral");
    expect(root).toHaveAttribute("data-full-width");
  });
});
