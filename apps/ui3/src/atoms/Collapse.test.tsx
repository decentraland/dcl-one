import { useState } from "react";
import { describe, expect, test } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import Collapse from "./Collapse";

function Section() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" aria-expanded={open} aria-controls="details" onClick={() => setOpen(!open)}>Details</button>
      <Collapse id="details" open={open}><a href="#more">More</a></Collapse>
    </>
  );
}

describe("Collapse", () => {
  test("closed content is hidden from the accessibility tree and the tab order", async () => {
    render(<Section />);
    const user = userEvent.setup();

    expect(screen.queryByRole("link", { name: "More" })).toBeNull();
    expect(document.getElementById("details")).toHaveAttribute("hidden");
    await user.tab();
    await user.tab();
    expect(screen.getByRole("button", { name: "Details" })).not.toHaveFocus();
    expect(document.activeElement).toBe(document.body);
  });

  test("the trigger opens and closes the content it controls", async () => {
    render(<Section />);
    const user = userEvent.setup();
    const trigger = screen.getByRole("button", { name: "Details" });

    await user.click(trigger);
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("link", { name: "More" })).toBeVisible();
    expect(document.getElementById(trigger.getAttribute("aria-controls") ?? "")).not.toHaveAttribute("hidden");

    await user.keyboard("{Enter}");
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("link", { name: "More" })).toBeNull();
  });

  test("a class of the caller is kept", () => {
    render(<Collapse open className="mine">Body</Collapse>);

    expect(screen.getByText("Body").parentElement).toHaveClass("mine");
  });
});
