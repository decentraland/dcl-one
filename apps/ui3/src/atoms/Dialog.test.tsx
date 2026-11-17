import { useRef, useState, type ReactNode } from "react";
import { afterEach, describe, expect, test, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import Dialog, { type DialogCloseReason, type DialogProps } from "./Dialog";
import { Menu, MenuItem } from "./Menu";
import { Select, SelectOption } from "./Select";

afterEach(() => {
  vi.restoreAllMocks();
});

type HarnessProps = Partial<DialogProps> & { onClose?: (reason: DialogCloseReason) => void; closable?: boolean; children?: ReactNode };

function Harness({ onClose, closable = true, children, ...props }: HarnessProps) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button onClick={() => setOpen(true)}>Open settings</button>
      <button>Elsewhere</button>
      <Dialog
        open={open} title="Settings"
        onClose={closable ? (reason) => { onClose?.(reason); setOpen(false); } : undefined}
        actions={<><button onClick={() => setOpen(false)}>Cancel</button><button>Save</button></>}
        {...props}
      >
        {children ?? <input aria-label="Scene name" />}
      </Dialog>
    </>
  );
}

const opener = () => screen.getByRole("button", { name: "Open settings" });
const dialog = () => screen.getByRole("dialog");
/** The dimmed area around the dialog. It has no role: a pointer reaches it, a screen reader does not. */
const backdrop = () => dialog().parentElement as HTMLElement;
const follows = (earlier: Element, later: Element) => !!(earlier.compareDocumentPosition(later) & Node.DOCUMENT_POSITION_FOLLOWING);

async function openDialog(props: HarnessProps = {}) {
  const user = userEvent.setup();
  render(<Harness {...props} />);
  await user.click(opener());
  return user;
}

describe("Dialog", () => {
  test("the test id of the caller is on the dialog", async () => {
    await openDialog({ "data-testid": "settings-dialog" });

    expect(screen.getByTestId("settings-dialog")).toBe(dialog());
  });

  test("a closed dialog renders nothing", () => {
    render(<Harness />);

    expect(screen.queryByRole("dialog")).toBeNull();
  });

  test("an open dialog is modal, takes its name from its title and sits outside the tree that rendered it", async () => {
    const user = userEvent.setup();
    const { container } = render(<Harness />);
    await user.click(opener());

    const node = screen.getByRole("dialog", { name: "Settings" });
    expect(node).toHaveAttribute("aria-modal", "true");
    expect(screen.getByRole("heading", { name: "Settings", level: 2 })).toBeInTheDocument();
    expect(container.contains(node)).toBe(false);
  });

  test("the title can be another heading level", async () => {
    await openDialog({ titleAs: "h5" });

    expect(screen.getByRole("heading", { name: "Settings", level: 5 })).toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: "Settings" })).toBeInTheDocument();
  });

  test("a dialog with no title takes the name the caller gives", async () => {
    await openDialog({ title: undefined, "aria-label": "Scene settings" });

    expect(screen.getByRole("dialog", { name: "Scene settings" })).toBeInTheDocument();
    expect(screen.queryByRole("heading")).toBeNull();
  });

  test("a dialog can be an alert dialog with a description", async () => {
    await openDialog({ role: "alertdialog", "aria-describedby": "why", children: <p id="why">This cannot be undone.</p> });

    expect(screen.getByRole("alertdialog", { name: "Settings" })).toHaveAccessibleDescription("This cannot be undone.");
  });

  test("the subtitle, the content and the actions are inside the dialog", async () => {
    await openDialog({ subtitle: "Applies to this scene" });

    const inside = within(dialog());
    expect(inside.getByText("Applies to this scene")).toBeInTheDocument();
    expect(inside.getByRole("textbox", { name: "Scene name" })).toBeInTheDocument();
    expect(inside.getByRole("button", { name: "Save" })).toBeInTheDocument();
  });

  test("focus moves to the dialog when it opens", async () => {
    await openDialog();

    expect(dialog()).toHaveFocus();
  });

  test("focus moves to the element the caller names", async () => {
    const user = userEvent.setup();
    function Named() {
      const [open, setOpen] = useState(false);
      const field = useRef<HTMLInputElement | null>(null);
      return (
        <>
          <button onClick={() => setOpen(true)}>Open settings</button>
          <Dialog open={open} title="Settings" initialFocus={field}><input ref={field} aria-label="Scene name" /></Dialog>
        </>
      );
    }
    render(<Named />);

    await user.click(opener());

    expect(screen.getByRole("textbox", { name: "Scene name" })).toHaveFocus();
  });

  test("a control that takes focus by itself keeps it", async () => {
    await openDialog({ children: <input aria-label="Scene name" autoFocus /> });

    expect(screen.getByRole("textbox", { name: "Scene name" })).toHaveFocus();
  });

  test("focus returns to the opener after a control in the dialog took focus by itself", async () => {
    const user = await openDialog({ children: <input aria-label="Scene name" autoFocus /> });
    expect(screen.getByRole("textbox", { name: "Scene name" })).toHaveFocus();

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(opener()).toHaveFocus();
  });

  test("Tab moves through the controls of the dialog and from the last to the first", async () => {
    const user = await openDialog();

    await user.tab();
    expect(screen.getByRole("button", { name: "close" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("textbox", { name: "Scene name" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "Save" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "close" })).toHaveFocus();
  });

  test("Shift+Tab moves from the first control to the last", async () => {
    const user = await openDialog();
    await user.tab();
    expect(screen.getByRole("button", { name: "close" })).toHaveFocus();

    await user.tab({ shift: true });

    expect(screen.getByRole("button", { name: "Save" })).toHaveFocus();
  });

  test("Shift+Tab from the dialog itself moves to the last control", async () => {
    const user = await openDialog();

    await user.tab({ shift: true });

    expect(screen.getByRole("button", { name: "Save" })).toHaveFocus();
  });

  test("Tab in a dialog with no control keeps focus on the dialog", async () => {
    const user = await openDialog({ closable: false, actions: undefined, children: <p>Saving the scene</p> });

    await user.tab();
    expect(dialog()).toHaveFocus();
    await user.tab({ shift: true });
    expect(dialog()).toHaveFocus();
  });

  test("Escape closes the dialog and focus returns to the opener", async () => {
    const onClose = vi.fn();
    const user = await openDialog({ onClose });
    await user.tab();
    await user.tab();

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("escape");
    expect(opener()).toHaveFocus();
  });

  test("the close button has a name, closes the dialog and focus returns to the opener", async () => {
    const onClose = vi.fn();
    const user = await openDialog({ onClose, closeLabel: "Close settings" });

    await user.click(screen.getByRole("button", { name: "Close settings" }));

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onClose).toHaveBeenCalledWith("close");
    expect(opener()).toHaveFocus();
  });

  test("a dialog the parent closes returns focus to the opener", async () => {
    const user = await openDialog();

    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(opener()).toHaveFocus();
  });

  test("the back button has a name and calls the caller", async () => {
    const onBack = vi.fn();
    const user = await openDialog({ onBack, backLabel: "Back to scenes" });

    await user.click(screen.getByRole("button", { name: "Back to scenes" }));

    expect(onBack).toHaveBeenCalledTimes(1);
    expect(dialog()).toBeInTheDocument();
  });

  test("a press on the backdrop closes the dialog", async () => {
    const onClose = vi.fn();
    const user = await openDialog({ onClose });

    await user.click(backdrop());

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("backdrop");
  });

  test("a press inside the dialog keeps it open", async () => {
    const onClose = vi.fn();
    const user = await openDialog({ onClose });

    await user.click(screen.getByRole("heading", { name: "Settings" }));
    await user.click(screen.getByRole("textbox", { name: "Scene name" }));

    expect(dialog()).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  test("a press that starts in the dialog and ends on the backdrop keeps it open", async () => {
    const onClose = vi.fn();
    await openDialog({ onClose });

    fireEvent.pointerDown(screen.getByRole("textbox", { name: "Scene name" }));
    fireEvent.click(backdrop());

    expect(dialog()).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  test("a dialog that does not close on the backdrop or on Escape stays open", async () => {
    const onClose = vi.fn();
    const user = await openDialog({ onClose, closeOnBackdrop: false, closeOnEscape: false });

    await user.click(backdrop());
    await user.keyboard("{Escape}");

    expect(dialog()).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  test("a dialog with no close handler has no close button and stays open", async () => {
    const user = await openDialog({ closable: false });

    await user.keyboard("{Escape}");
    await user.click(backdrop());

    expect(dialog()).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "close" })).toBeNull();
  });

  test("the page behind does not scroll while the dialog is open", async () => {
    const user = await openDialog();
    expect(document.body.style.overflow).toBe("hidden");

    await user.keyboard("{Escape}");

    expect(document.body.style.overflow).toBe("");
  });

  test("a dialog that does not lock the scroll leaves the page as it is", async () => {
    await openDialog({ lockScroll: false });

    expect(document.body.style.overflow).toBe("");
  });

  test("the page behind an open dialog is inert, and it is not once the dialog closes", async () => {
    const user = userEvent.setup();
    const { container } = render(<Harness />);
    await user.click(opener());

    expect(container).toHaveAttribute("inert");
    expect(backdrop()).not.toHaveAttribute("inert");

    await user.keyboard("{Escape}");

    expect(container).not.toHaveAttribute("inert");
    expect(opener()).toHaveFocus();
  });

  test("a dialog over another makes the lower one inert and gives it back on close", async () => {
    function Nested() {
      const [inner, setInner] = useState(false);
      return (
        <Dialog open title="Outer" onClose={() => {}}>
          <button onClick={() => setInner(true)}>Open inner</button>
          <Dialog open={inner} title="Inner" onClose={() => setInner(false)}>Inner content</Dialog>
        </Dialog>
      );
    }
    const user = userEvent.setup();
    const { container } = render(<Nested />);
    const outer = () => screen.getByRole("dialog", { name: "Outer" }).parentElement as HTMLElement;
    await user.click(screen.getByRole("button", { name: "Open inner" }));

    expect(outer()).toHaveAttribute("inert");
    expect(container).toHaveAttribute("inert");

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("dialog", { name: "Inner" })).toBeNull();
    expect(outer()).not.toHaveAttribute("inert");
    expect(container).toHaveAttribute("inert");
  });

  test("the page stays inert until the last open dialog closes, whatever order they close in", () => {
    const { container, rerender } = render(<><Dialog open title="First">One</Dialog><Dialog open title="Second">Two</Dialog></>);
    expect(container).toHaveAttribute("inert");

    rerender(<><Dialog open={false} title="First">One</Dialog><Dialog open title="Second">Two</Dialog></>);
    expect(container).toHaveAttribute("inert");

    rerender(<><Dialog open={false} title="First">One</Dialog><Dialog open={false} title="Second">Two</Dialog></>);
    expect(container).not.toHaveAttribute("inert");
  });

  test("an element that was inert before a dialog opened stays inert after it closes", () => {
    const outside = document.createElement("div");
    outside.setAttribute("inert", "");
    document.body.append(outside);
    try {
      const { rerender } = render(<Dialog open title="Only">Body</Dialog>);
      rerender(<Dialog open={false} title="Only">Body</Dialog>);
      expect(outside).toHaveAttribute("inert");
    } finally {
      outside.remove();
    }
  });

  test("Escape in a dialog that does not close on it does not reach the page behind", async () => {
    const page = vi.fn();
    document.addEventListener("keydown", page);
    try {
      const user = await openDialog({ closable: false });
      await user.keyboard("{Escape}");

      expect(dialog()).toBeInTheDocument();
      expect(page).not.toHaveBeenCalled();
    } finally {
      document.removeEventListener("keydown", page);
    }
  });

  test("a bare dialog draws only its children", async () => {
    await openDialog({ bare: true, "aria-label": "Preview", subtitle: "Hidden subtitle" });

    expect(screen.getByRole("dialog", { name: "Preview" })).toBeInTheDocument();
    expect(screen.queryByRole("heading")).toBeNull();
    expect(screen.queryByText("Hidden subtitle")).toBeNull();
    expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
    expect(screen.getByRole("textbox", { name: "Scene name" })).toBeInTheDocument();
  });
});

function NetworkField() {
  const [value, setValue] = useState("mainnet");
  return (
    <Select aria-label="Network" value={value} onChange={setValue}>
      <SelectOption value="mainnet">Mainnet</SelectOption>
      <SelectOption value="amoy">Amoy</SelectOption>
    </Select>
  );
}

function MoreMenu({ onClose }: { onClose?: (reason: string) => void }) {
  const [open, setOpen] = useState(false);
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  return (
    <>
      <button ref={setAnchor} onClick={() => setOpen((value) => !value)}>More</button>
      <Menu open={open} anchor={anchor} aria-label="More actions" onClose={(reason) => { onClose?.(reason); setOpen(false); }}>
        <MenuItem>Reset</MenuItem>
        <MenuItem>Export</MenuItem>
      </Menu>
    </>
  );
}

describe("a Select and a Menu opened from a Dialog", () => {
  test("the list of the select shows above the dialog", async () => {
    const user = await openDialog({ children: <NetworkField /> });

    await user.click(screen.getByRole("combobox", { name: "Network" }));

    const listbox = screen.getByRole("listbox", { name: "Network" });
    expect(dialog()).not.toContainElement(listbox);
    expect(follows(backdrop(), listbox)).toBe(true);
    expect(listbox).toHaveAttribute("data-over-modal");
  });

  test("the menu shows above the dialog", async () => {
    const user = await openDialog({ children: <MoreMenu /> });

    await user.click(screen.getByRole("button", { name: "More" }));

    const menu = screen.getByRole("menu", { name: "More actions" });
    expect(dialog()).not.toContainElement(menu);
    expect(follows(backdrop(), menu)).toBe(true);
    expect(menu.parentElement).toHaveAttribute("data-over-modal");
  });

  test("Escape closes only the select, and the next Escape closes the dialog", async () => {
    const onClose = vi.fn();
    const user = await openDialog({ onClose, children: <NetworkField /> });
    await user.click(screen.getByRole("combobox", { name: "Network" }));

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(dialog()).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("combobox", { name: "Network" })).toHaveFocus();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onClose).toHaveBeenCalledWith("escape");
    expect(opener()).toHaveFocus();
  });

  test("Escape closes only the menu, focus returns to its trigger, and the next Escape closes the dialog", async () => {
    const onClose = vi.fn();
    const user = await openDialog({ onClose, children: <MoreMenu /> });
    await user.click(screen.getByRole("button", { name: "More" }));
    expect(screen.getByRole("menuitem", { name: "Reset" })).toHaveFocus();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(dialog()).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "More" })).toHaveFocus();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(opener()).toHaveFocus();
  });

  test("the keys of the menu work while the dialog is open, and Tab returns focus to the trigger in the dialog", async () => {
    const user = await openDialog({ children: <MoreMenu /> });
    await user.click(screen.getByRole("button", { name: "More" }));

    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("menuitem", { name: "Export" })).toHaveFocus();
    await user.tab();

    expect(screen.queryByRole("menu")).toBeNull();
    expect(screen.getByRole("button", { name: "More" })).toHaveFocus();
    expect(dialog()).toBeInTheDocument();
  });

  test("a choice in the select keeps the dialog open", async () => {
    const onClose = vi.fn();
    const user = await openDialog({ onClose, children: <NetworkField /> });
    await user.click(screen.getByRole("combobox", { name: "Network" }));

    await user.click(screen.getByRole("option", { name: "Amoy" }));

    expect(screen.getByRole("combobox", { name: "Network" })).toHaveTextContent("Amoy");
    expect(dialog()).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  test("a press in the dialog closes only the menu", async () => {
    const onClose = vi.fn();
    const onMenuClose = vi.fn();
    const user = await openDialog({ onClose, children: <MoreMenu onClose={onMenuClose} /> });
    await user.click(screen.getByRole("button", { name: "More" }));

    await user.click(screen.getByRole("heading", { name: "Settings" }));

    expect(onMenuClose).toHaveBeenCalledWith("outside");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(dialog()).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  test("a press on the backdrop closes only the select, and the next press closes the dialog", async () => {
    const onClose = vi.fn();
    const user = await openDialog({ onClose, children: <NetworkField /> });
    await user.click(screen.getByRole("combobox", { name: "Network" }));

    await user.click(backdrop());
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(dialog()).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();

    await user.click(backdrop());
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onClose).toHaveBeenCalledWith("backdrop");
  });
});

/** A row menu: its item closes the menu and opens a dialog in the same update, so the item is gone when the dialog opens. */
function RowMenu() {
  const [menu, setMenu] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  return (
    <>
      <button ref={setAnchor} onClick={() => setMenu((value) => !value)}>Options for Plaza</button>
      <Menu open={menu} anchor={anchor} aria-label="Options for Plaza" modal onClose={() => setMenu(false)}>
        <MenuItem onClick={() => { setMenu(false); setConfirm(true); }}>Delete</MenuItem>
      </Menu>
      <Dialog open={confirm} title="Delete Plaza" onClose={() => setConfirm(false)}><p>This cannot be undone.</p></Dialog>
    </>
  );
}

/** Two steps of one flow: each step is its own dialog, and the next one replaces it in the same update. */
function TwoSteps() {
  const [step, setStep] = useState<"none" | "terms" | "contact">("none");
  const close = () => setStep("none");
  return (
    <>
      <button onClick={() => setStep("terms")}>Join</button>
      {step === "terms" ? <Dialog open title="Terms" onClose={close}><button onClick={() => setStep("contact")}>Accept</button></Dialog> : null}
      {step === "contact" ? <Dialog open title="Contact" onClose={close}><input aria-label="Email" /></Dialog> : null}
    </>
  );
}

describe("a dialog opened by a control that the same update removes", () => {
  test("a menu item opens a dialog, and focus returns to the menu trigger when the dialog closes", async () => {
    const user = userEvent.setup();
    render(<RowMenu />);
    await user.click(screen.getByRole("button", { name: "Options for Plaza" }));

    await user.click(screen.getByRole("menuitem", { name: "Delete" }));
    expect(screen.queryByRole("menu")).toBeNull();
    expect(screen.getByRole("dialog", { name: "Delete Plaza" })).toHaveFocus();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByRole("button", { name: "Options for Plaza" })).toHaveFocus();
  });

  test("a menu item chosen with the keyboard opens a dialog, and focus returns to the menu trigger when the dialog closes", async () => {
    const user = userEvent.setup();
    render(<RowMenu />);
    await user.click(screen.getByRole("button", { name: "Options for Plaza" }));
    expect(screen.getByRole("menuitem", { name: "Delete" })).toHaveFocus();

    await user.keyboard("{Enter}");
    expect(screen.getByRole("dialog", { name: "Delete Plaza" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "close" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByRole("button", { name: "Options for Plaza" })).toHaveFocus();
  });

  test("a step that replaces the dialog of the step before returns focus to the first opener when it closes", async () => {
    const user = userEvent.setup();
    render(<TwoSteps />);
    await user.click(screen.getByRole("button", { name: "Join" }));

    await user.click(screen.getByRole("button", { name: "Accept" }));
    expect(screen.getByRole("dialog", { name: "Contact" })).toHaveFocus();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByRole("button", { name: "Join" })).toHaveFocus();
  });
});
