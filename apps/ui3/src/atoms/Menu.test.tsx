import { useState } from "react";
import { afterEach, describe, expect, test, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { Menu, MenuItem, MenuSeparator, type MenuCloseReason, type MenuProps } from "./Menu";

afterEach(() => {
  vi.restoreAllMocks();
});

type HarnessProps = Partial<MenuProps> & {
  onClose?: (reason: MenuCloseReason) => void;
  onRename?: () => void;
  onDuplicate?: () => void;
  selected?: string;
};

function Harness({ onClose, onRename, onDuplicate, selected, ...props }: HarnessProps) {
  const [open, setOpen] = useState(false);
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  return (
    <>
      <button ref={setAnchor} aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((value) => !value)}>Actions</button>
      <button>Elsewhere</button>
      <p>Page text</p>
      <Menu open={open} anchor={anchor} aria-label="Scene actions" onClose={(reason) => { onClose?.(reason); setOpen(false); }} {...props}>
        <MenuItem onClick={onRename}>Rename</MenuItem>
        <MenuItem disabled onClick={onDuplicate}>Duplicate</MenuItem>
        <MenuItem selected={selected === "Delete"} tone="danger">Delete</MenuItem>
        <MenuSeparator />
        <MenuItem icon={<img alt="Folder glyph" />}>Reveal in folder</MenuItem>
      </Menu>
    </>
  );
}

const trigger = () => screen.getByRole("button", { name: "Actions" });
const item = (name: string) => screen.getByRole("menuitem", { name });

async function openMenu(props: HarnessProps = {}) {
  const user = userEvent.setup();
  render(<Harness {...props} />);
  await user.click(trigger());
  return user;
}

describe("Menu", () => {
  test("a closed menu renders nothing", () => {
    render(<Harness />);

    expect(screen.queryByRole("menu")).toBeNull();
    expect(screen.queryByRole("menuitem")).toBeNull();
  });

  test("an open menu has the menu role, its name, its items and its separator", async () => {
    await openMenu();

    const menu = screen.getByRole("menu", { name: "Scene actions" });
    expect(screen.getAllByRole("menuitem").map((node) => node.textContent)).toEqual(["Rename", "Duplicate", "Delete", "Reveal in folder"]);
    expect(menu).toContainElement(screen.getByRole("separator"));
    expect(item("Duplicate")).toBeDisabled();
  });

  test("the glyph of an item is not part of its name", async () => {
    await openMenu();

    expect(item("Reveal in folder")).toBeInTheDocument();
    expect(screen.queryByRole("img", { name: "Folder glyph" })).toBeNull();
  });

  test("focus moves to the first item when the menu opens", async () => {
    await openMenu();

    expect(item("Rename")).toHaveFocus();
  });

  test("focus moves to the selected item when the menu has one", async () => {
    await openMenu({ selected: "Delete" });

    expect(item("Delete")).toHaveFocus();
  });

  test("ArrowDown moves to the next item, skips a disabled one and goes from the last to the first", async () => {
    const user = await openMenu();

    await user.keyboard("{ArrowDown}");
    expect(item("Delete")).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(item("Reveal in folder")).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(item("Rename")).toHaveFocus();
  });

  test("ArrowUp moves to the previous item, skips a disabled one and goes from the first to the last", async () => {
    const user = await openMenu();

    await user.keyboard("{ArrowUp}");
    expect(item("Reveal in folder")).toHaveFocus();
    await user.keyboard("{ArrowUp}");
    expect(item("Delete")).toHaveFocus();
    await user.keyboard("{ArrowUp}");
    expect(item("Rename")).toHaveFocus();
  });

  test("End moves to the last item and Home to the first", async () => {
    const user = await openMenu();

    await user.keyboard("{End}");
    expect(item("Reveal in folder")).toHaveFocus();
    await user.keyboard("{Home}");
    expect(item("Rename")).toHaveFocus();
  });

  test("a letter moves to the next item that starts with it and never to a disabled one", async () => {
    const user = await openMenu();

    await user.keyboard("d");

    expect(item("Delete")).toHaveFocus();
  });

  test("the same letter after a pause moves on to the next match", async () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(1_000);
    const user = await openMenu();

    await user.keyboard("r");
    expect(item("Reveal in folder")).toHaveFocus();

    now.mockReturnValue(2_000);
    await user.keyboard("r");
    expect(item("Rename")).toHaveFocus();
  });

  test("letters typed in a row match the start of an item", async () => {
    vi.spyOn(Date, "now").mockReturnValue(1_000);
    const user = await openMenu();

    await user.keyboard("re");
    expect(item("Reveal in folder")).toHaveFocus();
    await user.keyboard("n");
    expect(item("Rename")).toHaveFocus();
  });

  test("a letter that matches no item leaves focus where it is", async () => {
    const user = await openMenu();

    await user.keyboard("x");

    expect(item("Rename")).toHaveFocus();
  });

  test("Enter and Space run the focused item", async () => {
    const onRename = vi.fn();
    const user = await openMenu({ onRename });

    await user.keyboard("{Enter}");
    expect(onRename).toHaveBeenCalledTimes(1);
    await user.keyboard(" ");
    expect(onRename).toHaveBeenCalledTimes(2);
    expect(screen.getByRole("menu")).toBeInTheDocument();
  });

  test("a menu that closes on select closes after the item ran and focus returns to the trigger", async () => {
    const onRename = vi.fn();
    const onClose = vi.fn();
    const user = await openMenu({ onRename, onClose, closeOnSelect: true });

    await user.click(item("Rename"));

    expect(onRename).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("select");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(trigger()).toHaveFocus();
  });

  test("a disabled item does not run", async () => {
    const onDuplicate = vi.fn();
    const onClose = vi.fn();
    await openMenu({ onDuplicate, onClose, closeOnSelect: true });

    // The styles stop a pointer on a disabled item, so the press comes as a plain click event.
    fireEvent.click(item("Duplicate"));

    expect(onDuplicate).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("menu")).toBeInTheDocument();
  });

  test("Escape closes the menu and focus returns to the trigger", async () => {
    const onClose = vi.fn();
    const user = await openMenu({ onClose });
    await user.keyboard("{ArrowDown}");

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("menu")).toBeNull();
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("escape");
    expect(trigger()).toHaveFocus();
  });

  test("Tab closes the menu and focus returns to the trigger", async () => {
    const onClose = vi.fn();
    const user = await openMenu({ onClose });

    await user.tab();

    expect(screen.queryByRole("menu")).toBeNull();
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("tab");
    expect(trigger()).toHaveFocus();
  });

  test("Shift+Tab closes the menu and focus returns to the trigger", async () => {
    const user = await openMenu();

    await user.tab({ shift: true });

    expect(screen.queryByRole("menu")).toBeNull();
    expect(trigger()).toHaveFocus();
  });

  test("a press outside closes the menu", async () => {
    const onClose = vi.fn();
    const user = await openMenu({ onClose });

    await user.click(screen.getByText("Page text"));

    expect(screen.queryByRole("menu")).toBeNull();
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("outside");
  });


  test("a modal menu stops the page scroll, and a press on the layer under it closes it", async () => {
    const onClose = vi.fn();
    const user = await openMenu({ onClose, modal: true });
    expect(document.body.style.overflow).toBe("hidden");

    await user.click(screen.getByRole("menu").parentElement?.previousElementSibling as HTMLElement);

    expect(screen.queryByRole("menu")).toBeNull();
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("outside");
    expect(document.body.style.overflow).toBe("");
    expect(trigger()).toHaveFocus();
  });

  test("a modal menu asked not to lock the scroll leaves the page free to scroll and still takes the press outside", async () => {
    const onClose = vi.fn();
    const user = await openMenu({ onClose, modal: true, lockScroll: false });
    expect(document.body.style.overflow).toBe("");

    await user.click(screen.getByRole("menu").parentElement?.previousElementSibling as HTMLElement);

    expect(screen.queryByRole("menu")).toBeNull();
    expect(onClose).toHaveBeenCalledWith("outside");
  });

  test("a menu that is not modal leaves the page free to scroll", async () => {
    await openMenu();

    expect(document.body.style.overflow).toBe("");
  });

  test("a press on another control closes the menu and focus stays on that control", async () => {
    const user = await openMenu();

    await user.click(screen.getByRole("button", { name: "Elsewhere" }));

    expect(screen.queryByRole("menu")).toBeNull();
    expect(screen.getByRole("button", { name: "Elsewhere" })).toHaveFocus();
  });

  test("a press on the trigger of an open menu closes it once", async () => {
    const onClose = vi.fn();
    const user = await openMenu({ onClose });

    await user.click(trigger());

    expect(screen.queryByRole("menu")).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
    expect(trigger()).toHaveAttribute("aria-expanded", "false");
  });

  test("a menu with no enabled item takes focus itself, so the keys still close it", async () => {
    const user = userEvent.setup();
    function Empty() {
      const [open, setOpen] = useState(false);
      const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
      return (
        <>
          <button ref={setAnchor} onClick={() => setOpen(true)}>Actions</button>
          <Menu open={open} anchor={anchor} aria-label="Scene actions" onClose={() => setOpen(false)}>
            <MenuItem disabled>Rename</MenuItem>
          </Menu>
        </>
      );
    }
    render(<Empty />);
    await user.click(trigger());
    expect(screen.getByRole("menu")).toHaveFocus();

    await user.keyboard("{ArrowDown}{Escape}");

    expect(screen.queryByRole("menu")).toBeNull();
    expect(trigger()).toHaveFocus();
  });

  test("a menu can take its name from another element", async () => {
    const user = userEvent.setup();
    function Labelled() {
      const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
      return (
        <>
          <button ref={setAnchor} id="sort-trigger">Sort by</button>
          <Menu open anchor={anchor} aria-labelledby="sort-trigger"><MenuItem>Newest</MenuItem></Menu>
        </>
      );
    }
    render(<Labelled />);
    await user.tab();

    expect(screen.getByRole("menu", { name: "Sort by" })).toBeInTheDocument();
  });
});
