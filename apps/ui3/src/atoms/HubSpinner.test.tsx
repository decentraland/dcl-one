import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vitest";
import { render, screen } from "@testing-library/react";

import HubSpinner from "./HubSpinner";
import styles from "./HubSpinner.module.css";

describe("HubSpinner", () => {
  test("with no props it is the spinner of ui3", () => {
    render(<HubSpinner size={20} />);

    const spinner = screen.getByRole("status", { name: "Loading" });
    expect(spinner).toHaveClass("spinner");
    expect(spinner.style.getPropertyValue("--sz")).toBe("20px");
    expect(spinner.style.getPropertyValue("--spinner-arc")).toBe("");
    expect(spinner.querySelector("svg")).toHaveAttribute("width", "20");
  });

  test("role progressbar with a label has a name and no value", () => {
    render(<HubSpinner role="progressbar" label="Publishing the scene" />);

    const spinner = screen.getByRole("progressbar", { name: "Publishing the scene" });
    expect(spinner).not.toHaveAttribute("aria-valuenow");
  });

  test("the colour keywords map to the text colour and the brand colour", () => {
    render(<><HubSpinner color="inherit" label="A" /><HubSpinner color="primary" label="B" /><HubSpinner color="#fff" label="C" /></>);

    expect(screen.getByRole("status", { name: "A" }).style.getPropertyValue("--spinner-arc")).toBe("currentColor");
    expect(screen.getByRole("status", { name: "B" }).style.getPropertyValue("--spinner-arc")).toBe("var(--brand)");
    expect(screen.getByRole("status", { name: "C" }).style.getPropertyValue("--spinner-arc")).toBe("#fff");
  });

  test("aria-label and data attributes reach the spinner", () => {
    render(<HubSpinner aria-label="Saving" data-testid="saving" />);

    expect(screen.getByRole("status", { name: "Saving" })).toHaveAttribute("data-testid", "saving");
  });

  test("the wide ring and a class of the caller stand beside the class of spinner.css", () => {
    render(<><HubSpinner ring="wide" label="Wide" /><HubSpinner className="mine" label="Mine" /></>);

    const wide = screen.getByRole("status", { name: "Wide" });
    expect(styles.wide).toBeTruthy();
    expect(wide).toHaveClass("spinner", styles.wide ?? "");
    expect(wide).not.toHaveAttribute("ring");
    expect(screen.getByRole("status", { name: "Mine" })).toHaveClass("spinner", "mine");
    expect(screen.getByRole("status", { name: "Mine" })).not.toHaveClass(styles.wide ?? "");
  });

  test("the wide ring changes the turn and the arc, and a spinner without it keeps the ring of ui3", () => {
    render(<><HubSpinner ring="wide" label="Wide" /><HubSpinner label="Plain" /></>);

    const arc = (name: string) => getComputedStyle(screen.getByRole("status", { name }).querySelector("circle")!);
    expect(arc("Wide").getPropertyValue("stroke-width")).toBe("4.091px");
    expect(arc("Wide").getPropertyValue("animation")).toContain("1.4s");
    expect(arc("Plain").getPropertyValue("stroke-width")).not.toBe("4.091px");
    expect(arc("Plain").getPropertyValue("animation")).toBe("");
  });

  // Spinner and its other users must not change: no rule of the module may match an element without a module class.
  test("every rule of the module names a class of the module", () => {
    const css = readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), "HubSpinner.module.css"), "utf8")
      .replace(/\/\*[\s\S]*?\*\//g, "")
      .replace(/@keyframes[^{]*\{(?:[^{}]*\{[^{}]*\})*[^{}]*\}/g, "");
    const selectors = [...css.matchAll(/([^{}]+)\{/g)].flatMap(([, list = ""]) => list.split(",").map((s) => s.trim()));

    expect(selectors.length).toBe(2);
    for (const selector of selectors) expect(selector.replace(/:global\([^)]*\)/g, "")).toMatch(/\.wide\b/);
    expect(css).not.toContain("!important");
  });
});
