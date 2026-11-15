import { describe, expect, test } from "vitest";
import { render, screen } from "@testing-library/react";

import Spinner from "./Spinner";

describe("Spinner", () => {
  test("it keeps its role, name and size", () => {
    render(<Spinner size={20} />);

    const spinner = screen.getByRole("status", { name: "Loading" });
    expect(spinner).toHaveClass("spinner");
    expect(spinner.style.getPropertyValue("--sz")).toBe("20px");
    expect(spinner.style.getPropertyValue("--spinner-arc")).toBe("");
    expect(spinner.querySelector("svg")).toHaveAttribute("width", "20");
  });

  test("a colour and the attributes of the caller reach the spinner", () => {
    render(<Spinner color="#fff" role="progressbar" aria-label="Publishing the scene" />);

    const spinner = screen.getByRole("progressbar", { name: "Publishing the scene" });
    expect(spinner.style.getPropertyValue("--spinner-arc")).toBe("#fff");
  });
});
