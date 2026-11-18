import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vitest";
import { render, screen } from "@testing-library/react";

import Spinner from "./Spinner";
import styles from "./Spinner.module.css";

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
    render(<Spinner color="#fff" role="progressbar" aria-label="Publishing the scene" data-testid="publishing" />);

    const spinner = screen.getByRole("progressbar", { name: "Publishing the scene" });
    expect(spinner.style.getPropertyValue("--spinner-arc")).toBe("#fff");
    expect(spinner).toHaveAttribute("data-testid", "publishing");
    expect(spinner).not.toHaveAttribute("aria-valuenow");
  });

  test("a label names the spinner", () => {
    render(<Spinner label="Saving" />);

    expect(screen.getByRole("status", { name: "Saving" })).toBeInTheDocument();
  });

  test("the colour keywords map to the text colour and the brand colour", () => {
    render(<><Spinner color="inherit" label="A" /><Spinner color="primary" label="B" /></>);

    expect(screen.getByRole("status", { name: "A" }).style.getPropertyValue("--spinner-arc")).toBe("currentColor");
    expect(screen.getByRole("status", { name: "B" }).style.getPropertyValue("--spinner-arc")).toBe("var(--brand)");
  });

  test("a class and a style of the caller stand beside those of the spinner", () => {
    render(<Spinner size={16} className="mine" style={{ margin: "4px" }} label="Mine" />);

    const spinner = screen.getByRole("status", { name: "Mine" });
    expect(spinner).toHaveClass("spinner", "mine");
    expect(spinner.style.margin).toBe("4px");
    expect(spinner.style.getPropertyValue("--sz")).toBe("16px");
  });

  test("the wide ring changes the turn and the arc, and a spinner without it keeps its ring", () => {
    render(<><Spinner ring="wide" label="Wide" /><Spinner label="Plain" /></>);

    const wide = screen.getByRole("status", { name: "Wide" });
    expect(wide).toHaveClass("spinner", styles.wide ?? "");
    expect(wide).not.toHaveAttribute("ring");
    const arc = (name: string) => getComputedStyle(screen.getByRole("status", { name }).querySelector("circle")!);
    expect(arc("Wide").getPropertyValue("stroke-width")).toBe("4.091px");
    expect(arc("Wide").getPropertyValue("animation")).toContain("1.4s");
    expect(arc("Plain").getPropertyValue("stroke-width")).not.toBe("4.091px");
    expect(arc("Plain").getPropertyValue("animation")).toBe("");
  });

  // The other users of Spinner must not change: no rule of the module may match an element without a module class.
  test("every rule of the module names a class of the module", () => {
    const css = readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), "Spinner.module.css"), "utf8")
      .replace(/\/\*[\s\S]*?\*\//g, "")
      .replace(/@keyframes[^{]*\{(?:[^{}]*\{[^{}]*\})*[^{}]*\}/g, "");
    const selectors = [...css.matchAll(/([^{}]+)\{/g)].flatMap(([, list = ""]) => list.split(",").map((s) => s.trim()));

    expect(selectors.length).toBe(2);
    for (const selector of selectors) expect(selector.replace(/:global\([^)]*\)/g, "")).toMatch(/\.wide\b/);
    expect(css).not.toContain("!important");
  });
});
