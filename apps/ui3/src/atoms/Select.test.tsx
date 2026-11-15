import { useState, type ReactNode } from "react";
import { afterEach, describe, expect, test, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { Select, SelectOption } from "./Select";

afterEach(() => {
  vi.restoreAllMocks();
});

type Network = "mainnet" | "sepolia" | "amoy" | "polygon";

type SingleProps = {
  initial?: Network | "";
  onChange?: (value: Network) => void;
  onOpenChange?: (open: boolean) => void;
  disabled?: boolean;
  error?: boolean;
  name?: string;
  renderValue?: (value: Network) => ReactNode;
  modal?: boolean;
  lockScroll?: boolean;
};

const options = [
  <SelectOption key="mainnet" value="mainnet">Mainnet</SelectOption>,
  <SelectOption key="sepolia" value="sepolia" disabled>Sepolia</SelectOption>,
  <SelectOption key="amoy" value="amoy">Amoy</SelectOption>,
  <SelectOption key="polygon" value="polygon">Polygon</SelectOption>,
];

function Single({ initial = "", onChange, ...props }: SingleProps) {
  const [value, setValue] = useState<Network | "">(initial);
  return (
    <>
      <Select<Network> aria-label="Network" placeholder="Pick a network" value={value} onChange={(next) => { onChange?.(next); setValue(next); }} {...props}>
        {options}
      </Select>
      <button>Elsewhere</button>
      <p>Page text</p>
    </>
  );
}

function Multiple({ initial = [], onChange, maxSelected }: { initial?: Network[]; onChange?: (value: Network[]) => void; maxSelected?: number }) {
  const [value, setValue] = useState<Network[]>(initial);
  return (
    <Select<Network> multiple aria-label="Networks" placeholder="Pick networks" value={value} maxSelected={maxSelected} onChange={(next) => { onChange?.(next); setValue(next); }}>
      {options}
    </Select>
  );
}

const combobox = () => screen.getByRole("combobox");
const option = (name: string) => screen.getByRole("option", { name });
/** The option the keys point at: the trigger names it, and focus stays on the trigger. */
const active = () => {
  const id = combobox().getAttribute("aria-activedescendant");
  return id ? screen.getAllByRole("option").find((node) => node.id === id)?.textContent : undefined;
};

async function openSingle(props: SingleProps = {}) {
  const user = userEvent.setup();
  render(<Single {...props} />);
  await user.click(combobox());
  return user;
}

describe("Select", () => {
  test("a closed select is a collapsed combobox that says it opens a listbox", () => {
    render(<Single />);

    const trigger = screen.getByRole("combobox", { name: "Network" });
    expect(trigger).toHaveAttribute("aria-haspopup", "listbox");
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(trigger).not.toHaveAttribute("aria-controls");
    expect(trigger).toHaveTextContent("Pick a network");
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  test("an open select names its listbox, and the listbox holds the options with their state", async () => {
    await openSingle({ initial: "amoy" });

    const listbox = screen.getByRole("listbox", { name: "Network" });
    expect(combobox()).toHaveAttribute("aria-expanded", "true");
    expect(combobox()).toHaveAttribute("aria-controls", listbox.id);
    expect(screen.getAllByRole("option").map((node) => node.textContent)).toEqual(["Mainnet", "Sepolia", "Amoy", "Polygon"]);
    expect(option("Amoy")).toHaveAttribute("aria-selected", "true");
    expect(option("Mainnet")).toHaveAttribute("aria-selected", "false");
    expect(option("Sepolia")).toHaveAttribute("aria-disabled", "true");
    expect(listbox).not.toHaveAttribute("aria-multiselectable");
  });

  test("focus stays on the trigger while the list is open, and the keys start at the selected option", async () => {
    await openSingle({ initial: "amoy" });

    expect(combobox()).toHaveFocus();
    expect(active()).toBe("Amoy");
  });

  test("with no value the keys start at the first enabled option", async () => {
    await openSingle();

    expect(active()).toBe("Mainnet");
  });

  test("a label element names the trigger", () => {
    render(
      <>
        <label htmlFor="network">Network</label>
        <Select id="network" value="mainnet">{options}</Select>
      </>,
    );

    expect(screen.getByLabelText("Network")).toBe(combobox());
    expect(combobox()).toHaveTextContent("Mainnet");
  });

  test("ArrowDown moves to the next option, skips a disabled one and stops at the last", async () => {
    const user = await openSingle();

    await user.keyboard("{ArrowDown}");
    expect(active()).toBe("Amoy");
    await user.keyboard("{ArrowDown}");
    expect(active()).toBe("Polygon");
    await user.keyboard("{ArrowDown}");
    expect(active()).toBe("Polygon");
  });

  test("ArrowUp moves to the previous option, skips a disabled one and stops at the first", async () => {
    const user = await openSingle({ initial: "polygon" });

    await user.keyboard("{ArrowUp}");
    expect(active()).toBe("Amoy");
    await user.keyboard("{ArrowUp}");
    expect(active()).toBe("Mainnet");
    await user.keyboard("{ArrowUp}");
    expect(active()).toBe("Mainnet");
  });

  test("End moves to the last option and Home to the first", async () => {
    const user = await openSingle({ initial: "amoy" });

    await user.keyboard("{End}");
    expect(active()).toBe("Polygon");
    await user.keyboard("{Home}");
    expect(active()).toBe("Mainnet");
  });

  test("a letter moves to the next option that starts with it", async () => {
    const user = await openSingle();

    await user.keyboard("p");

    expect(active()).toBe("Polygon");
  });

  test("a letter never moves to a disabled option", async () => {
    const user = await openSingle();

    await user.keyboard("s");

    expect(active()).toBe("Mainnet");
  });

  test("letters typed in a row match the start of an option, and a pause starts again", async () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(1_000);
    const user = userEvent.setup();
    render(
      <Select aria-label="Size" value="">
        <SelectOption value="s">Small</SelectOption>
        <SelectOption value="m">Medium</SelectOption>
        <SelectOption value="x">Massive</SelectOption>
      </Select>,
    );
    await user.click(combobox());

    await user.keyboard("ma");
    expect(active()).toBe("Massive");

    now.mockReturnValue(2_000);
    await user.keyboard("m");
    expect(active()).toBe("Medium");
  });

  test("Enter takes the option the keys point at, closes the list and shows the choice", async () => {
    const onChange = vi.fn();
    const user = await openSingle({ onChange });

    await user.keyboard("{ArrowDown}{Enter}");

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith("amoy");
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(combobox()).toHaveTextContent("Amoy");
    expect(combobox()).toHaveFocus();
  });

  test("Space takes the option the keys point at and the list stays closed", async () => {
    const onChange = vi.fn();
    const user = await openSingle({ onChange });

    await user.keyboard("{End} ");

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith("polygon");
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  test("Enter on the option that is already selected closes the list with no change", async () => {
    const onChange = vi.fn();
    const user = await openSingle({ initial: "amoy", onChange });

    await user.keyboard("{Enter}");

    expect(onChange).not.toHaveBeenCalled();
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  test("Escape closes the list with no change and focus stays on the trigger", async () => {
    const onChange = vi.fn();
    const user = await openSingle({ onChange });
    await user.keyboard("{ArrowDown}");

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("listbox")).toBeNull();
    expect(onChange).not.toHaveBeenCalled();
    expect(combobox()).toHaveAttribute("aria-expanded", "false");
    expect(combobox()).toHaveFocus();
  });

  test("Tab closes the list with no change and focus moves on", async () => {
    const onChange = vi.fn();
    const user = await openSingle({ onChange });

    await user.tab();

    expect(screen.queryByRole("listbox")).toBeNull();
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Elsewhere" })).toHaveFocus();
  });

  test("a press outside closes the list", async () => {
    const user = await openSingle();

    await user.click(screen.getByText("Page text"));

    expect(screen.queryByRole("listbox")).toBeNull();
  });

  test("a press on the backdrop of a modal select closes the list with no change and focus stays on the trigger", async () => {
    const onChange = vi.fn();
    const user = await openSingle({ modal: true, onChange });
    const backdrop = screen.getByRole("listbox").previousElementSibling as HTMLElement;
    expect(backdrop).toHaveAttribute("aria-hidden", "true");

    await user.click(backdrop);

    expect(screen.queryByRole("listbox")).toBeNull();
    expect(onChange).not.toHaveBeenCalled();
    expect(combobox()).toHaveFocus();
  });

  test("a modal select keeps its keys and its choice by a press", async () => {
    const onChange = vi.fn();
    const user = await openSingle({ modal: true, onChange });

    await user.keyboard("{ArrowDown}");
    expect(active()).toBe("Amoy");
    await user.click(option("Polygon"));

    expect(onChange).toHaveBeenCalledWith("polygon");
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(combobox()).toHaveFocus();
  });

  test("a select that locks the scroll stops the page behind while the list is open", async () => {
    const user = await openSingle({ lockScroll: true });
    expect(document.body.style.overflow).toBe("hidden");

    await user.keyboard("{Escape}");

    expect(document.body.style.overflow).toBe("");
  });

  test("a select leaves the page scroll as it is unless it is asked to lock it", async () => {
    await openSingle();

    expect(document.body.style.overflow).toBe("");
  });

  test("a press on the trigger of an open select closes the list", async () => {
    const onOpenChange = vi.fn();
    const user = await openSingle({ onOpenChange });

    await user.click(combobox());

    expect(screen.queryByRole("listbox")).toBeNull();
    expect(onOpenChange.mock.calls).toEqual([[true], [false]]);
  });

  test("a press on an option takes it and closes the list", async () => {
    const onChange = vi.fn();
    const user = await openSingle({ onChange });

    await user.click(option("Polygon"));

    expect(onChange).toHaveBeenCalledWith("polygon");
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(combobox()).toHaveTextContent("Polygon");
    expect(combobox()).toHaveFocus();
  });

  test("a press on a disabled option does nothing", async () => {
    const onChange = vi.fn();
    await openSingle({ onChange });

    // The styles stop a pointer on a disabled option, so the press comes as a plain click event.
    fireEvent.click(option("Sepolia"));

    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByRole("listbox")).toBeInTheDocument();
  });

  test.each([
    ["{Enter}", "Amoy"],
    [" ", "Amoy"],
    ["{ArrowDown}", "Amoy"],
    ["{ArrowUp}", "Amoy"],
    ["{Home}", "Mainnet"],
    ["{End}", "Polygon"],
    ["p", "Polygon"],
  ])("the key %s opens a closed select", async (key, start) => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Single initial="amoy" onChange={onChange} />);
    await user.tab();
    expect(combobox()).toHaveFocus();

    await user.keyboard(key);

    expect(screen.getByRole("listbox")).toBeInTheDocument();
    expect(active()).toBe(start);
    expect(onChange).not.toHaveBeenCalled();
  });

  test("a letter that matches no option leaves a closed select closed", async () => {
    const user = userEvent.setup();
    render(<Single />);
    await user.tab();

    await user.keyboard("z");

    expect(screen.queryByRole("listbox")).toBeNull();
  });

  test("a disabled select does not open", async () => {
    const user = userEvent.setup();
    render(<Single disabled />);

    await user.click(combobox());

    expect(combobox()).toBeDisabled();
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  test("a select with an error says so", () => {
    render(<Single error />);

    expect(combobox()).toHaveAttribute("aria-invalid", "true");
  });

  test("the caller can draw the value", () => {
    render(<Single initial="polygon" renderValue={(value) => <strong>Chain: {value}</strong>} />);

    expect(combobox()).toHaveTextContent("Chain: polygon");
  });

  test("a named select sends its value with a form", async () => {
    const user = userEvent.setup();
    render(<form aria-label="Deploy"><Single name="network" initial="mainnet" /></form>);
    const form = screen.getByRole<HTMLFormElement>("form", { name: "Deploy" });
    expect(new FormData(form).get("network")).toBe("mainnet");

    await user.click(combobox());
    await user.click(option("Amoy"));

    expect(new FormData(form).get("network")).toBe("amoy");
  });

  test("a parent that owns the open state decides when the list shows", async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    const { rerender } = render(<Select aria-label="Network" value="amoy" open={false} onOpenChange={onOpenChange}>{options}</Select>);

    await user.click(combobox());
    expect(onOpenChange.mock.calls).toEqual([[true]]);
    expect(screen.queryByRole("listbox")).toBeNull();

    rerender(<Select aria-label="Network" value="amoy" open onOpenChange={onOpenChange}>{options}</Select>);
    expect(screen.getByRole("listbox")).toBeInTheDocument();
    expect(active()).toBe("Amoy");

    await user.keyboard("{Escape}");
    expect(onOpenChange.mock.calls).toEqual([[true], [false]]);
    expect(screen.getByRole("listbox")).toBeInTheDocument();
  });

  test("a multiple select marks its listbox, keeps the list open and adds and removes choices", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Multiple initial={["mainnet"]} onChange={onChange} />);
    await user.click(combobox());
    expect(screen.getByRole("listbox", { name: "Networks" })).toHaveAttribute("aria-multiselectable", "true");

    await user.click(option("Polygon"));
    expect(onChange).toHaveBeenLastCalledWith(["mainnet", "polygon"]);
    expect(option("Polygon")).toHaveAttribute("aria-selected", "true");
    expect(combobox()).toHaveTextContent("Mainnet, Polygon");

    await user.keyboard("{Home}{Enter}");
    expect(onChange).toHaveBeenLastCalledWith(["polygon"]);
    expect(option("Mainnet")).toHaveAttribute("aria-selected", "false");
    expect(screen.getByRole("listbox")).toBeInTheDocument();
  });

  test("a multiple select ignores a choice over its limit and still removes one", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Multiple initial={["mainnet", "amoy"]} maxSelected={2} onChange={onChange} />);
    await user.click(combobox());

    await user.click(option("Polygon"));
    expect(onChange).not.toHaveBeenCalled();

    await user.click(option("Amoy"));
    expect(onChange).toHaveBeenLastCalledWith(["mainnet"]);
  });

  test("a multiple select with no choice shows its placeholder", () => {
    render(<Multiple />);

    expect(combobox()).toHaveTextContent("Pick networks");
  });
});
