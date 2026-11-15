import { useState } from "react";
import { describe, expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import Radio, { RadioGroup } from "./Radio";

const options = (
  <>
    <Radio value="public">Public</Radio>
    <Radio value="private">Private</Radio>
    <Radio value="invite">Invite only</Radio>
  </>
);

describe("RadioGroup", () => {
  test("a labelled group is a radiogroup with that name and one radio per option", () => {
    render(<RadioGroup label="Access" defaultValue="private">{options}</RadioGroup>);

    expect(screen.getByRole("radiogroup", { name: "Access" })).toBeInTheDocument();
    expect(screen.getAllByRole("radio")).toHaveLength(3);
    expect(screen.getByRole("radio", { name: "Private" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "Public" })).not.toBeChecked();
  });

  test("aria-label names a group with no visible label", () => {
    render(<RadioGroup aria-label="Access">{options}</RadioGroup>);

    expect(screen.getByRole("radiogroup", { name: "Access" })).toBeInTheDocument();
  });

  test("a click selects the option and reports its value", async () => {
    const onChange = vi.fn();
    render(<RadioGroup aria-label="Access" defaultValue="public" onChange={onChange}>{options}</RadioGroup>);

    await userEvent.setup().click(screen.getByRole("radio", { name: "Invite only" }));

    expect(screen.getByRole("radio", { name: "Invite only" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "Public" })).not.toBeChecked();
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange.mock.calls[0]?.[0]).toBe("invite");
  });

  test("the arrow keys move the selection inside the group", async () => {
    render(<RadioGroup aria-label="Access" defaultValue="public">{options}</RadioGroup>);
    const user = userEvent.setup();

    await user.tab();
    expect(screen.getByRole("radio", { name: "Public" })).toHaveFocus();
    await user.keyboard("{ArrowDown}");

    expect(screen.getByRole("radio", { name: "Private" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "Private" })).toHaveFocus();
  });

  test("a controlled group shows the value the owner keeps", async () => {
    function Owner() {
      const [value, setValue] = useState("public");
      return <RadioGroup aria-label="Access" value={value} onChange={(next) => setValue(next === "invite" ? "public" : next)}>{options}</RadioGroup>;
    }
    render(<Owner />);
    const user = userEvent.setup();

    await user.click(screen.getByRole("radio", { name: "Private" }));
    expect(screen.getByRole("radio", { name: "Private" })).toBeChecked();
    await user.click(screen.getByRole("radio", { name: "Invite only" }));
    expect(screen.getByRole("radio", { name: "Public" })).toBeChecked();
  });

  test("a disabled group disables every radio", async () => {
    const onChange = vi.fn();
    render(<RadioGroup aria-label="Access" disabled onChange={onChange}>{options}</RadioGroup>);

    await userEvent.setup().click(screen.getByRole("radio", { name: "Private" }));

    for (const radio of screen.getAllByRole("radio")) expect(radio).toBeDisabled();
    expect(onChange).not.toHaveBeenCalled();
  });

  test("one disabled radio stays out while the others work", async () => {
    render(
      <RadioGroup aria-label="Access">
        <Radio value="a">A</Radio>
        <Radio value="b" disabled>B</Radio>
      </RadioGroup>,
    );

    await userEvent.setup().click(screen.getByRole("radio", { name: "B" }));

    expect(screen.getByRole("radio", { name: "B" })).toBeDisabled();
    expect(screen.getByRole("radio", { name: "B" })).not.toBeChecked();
    expect(screen.getByRole("radio", { name: "A" })).toBeEnabled();
  });
});

describe("Radio", () => {
  test("outside a group the caller owns the state", async () => {
    const onChange = vi.fn();
    render(<Radio value="x" name="solo" checked={false} onChange={onChange}>Solo</Radio>);

    await userEvent.setup().click(screen.getByRole("radio", { name: "Solo" }));

    expect(onChange.mock.calls[0]?.[0]).toBe("x");
    expect(screen.getByRole("radio", { name: "Solo" })).not.toBeChecked();
  });
});
