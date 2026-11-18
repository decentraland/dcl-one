import { describe, expect, test } from "vitest";
import { render, screen } from "@testing-library/react";

import ProgressBar from "./ProgressBar";

describe("ProgressBar", () => {
  test("a determinate bar exposes its value, its range and its name", () => {
    render(<ProgressBar value={40} aria-label="Upload" />);

    const bar = screen.getByRole("progressbar", { name: "Upload" });
    expect(bar).toHaveAttribute("aria-valuenow", "40");
    expect(bar).toHaveAttribute("aria-valuemin", "0");
    expect(bar).toHaveAttribute("aria-valuemax", "100");
    expect((bar.firstElementChild as HTMLElement).style.getPropertyValue("--progress")).toBe("40%");
  });

  test("a value out of range is kept inside it", () => {
    render(<><ProgressBar value={140} aria-label="High" /><ProgressBar value={-5} aria-label="Low" /></>);

    expect(screen.getByRole("progressbar", { name: "High" })).toHaveAttribute("aria-valuenow", "100");
    expect(screen.getByRole("progressbar", { name: "Low" })).toHaveAttribute("aria-valuenow", "0");
  });

  test("an indeterminate bar has no value", () => {
    render(<ProgressBar indeterminate aria-label="Publishing" />);

    const bar = screen.getByRole("progressbar", { name: "Publishing" });
    expect(bar).not.toHaveAttribute("aria-valuenow");
    expect(bar).toHaveAttribute("data-indeterminate");
  });

  test("aria-labelledby names the bar and danger is set on the root", () => {
    render(<><span id="p">Storage</span><ProgressBar value={90} danger aria-labelledby="p" /></>);

    expect(screen.getByRole("progressbar", { name: "Storage" })).toHaveAttribute("data-danger");
  });
});
