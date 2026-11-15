import { afterEach, describe, expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import Toggle from "./Toggle";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllEnvs();
});

describe("Toggle", () => {
  test("a call with the props ui3 uses today keeps its markup", () => {
    const { container } = render(<Toggle ariaLabel="Mute" defaultChecked />);

    expect(container.innerHTML).toBe(
      '<button type="button" role="switch" aria-checked="true" aria-label="Mute" class="toggle is-on"><span class="toggle__knob"></span></button>',
    );
  });

  test("it is a switch with a name that click, Space and Enter toggle", async () => {
    const onChange = vi.fn();
    render(<Toggle ariaLabel="Mute" onChange={onChange} />);
    const user = userEvent.setup();

    const toggle = screen.getByRole("switch", { name: "Mute" });
    expect(toggle).not.toBeChecked();
    await user.click(toggle);
    expect(toggle).toBeChecked();
    await user.keyboard(" ");
    expect(toggle).not.toBeChecked();
    await user.keyboard("{Enter}");
    expect(toggle).toBeChecked();

    expect(onChange.mock.calls.map((call) => call[0])).toEqual([true, false, true]);
  });

  test("ariaLabelledBy names the switch", () => {
    render(<><span id="l">Auto save</span><Toggle ariaLabelledBy="l" /></>);

    expect(screen.getByRole("switch", { name: "Auto save" })).toBeInTheDocument();
  });

  test("a disabled switch is disabled and does not change", async () => {
    const onChange = vi.fn();
    render(<Toggle ariaLabel="Mute" disabled onChange={onChange} />);

    const toggle = screen.getByRole("switch", { name: "Mute" });
    await userEvent.setup().click(toggle);

    expect(toggle).toBeDisabled();
    expect(toggle).not.toBeChecked();
    expect(onChange).not.toHaveBeenCalled();
  });

  test("a controlled switch shows the value of its owner", async () => {
    const onChange = vi.fn();
    render(<Toggle ariaLabel="Mute" checked={false} onChange={onChange} />);

    await userEvent.setup().click(screen.getByRole("switch", { name: "Mute" }));

    expect(onChange).toHaveBeenCalledWith(true);
    expect(screen.getByRole("switch", { name: "Mute" })).not.toBeChecked();
  });

  test("id and a class of the caller reach the button", () => {
    render(<Toggle ariaLabel="Mute" id="t" className="mine" />);

    const toggle = screen.getByRole("switch", { name: "Mute" });
    expect(toggle).toHaveAttribute("id", "t");
    expect(toggle).toHaveClass("toggle", "mine");
  });

  test("a development build warns about a switch with no name and a production build does not", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    vi.stubEnv("DEV", true);
    render(<Toggle />);
    expect(warn).toHaveBeenCalledTimes(1);

    vi.stubEnv("DEV", false);
    render(<Toggle />);
    expect(warn).toHaveBeenCalledTimes(1);
  });
});
