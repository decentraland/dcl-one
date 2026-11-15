import { useRef, useState } from "react";
import { afterEach, describe, expect, test, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import Tooltip from "./Tooltip";

const box = (left: number, top: number, width: number, height: number) =>
  ({ left, top, width, height, right: left + width, bottom: top + height, x: left, y: top, toJSON: () => ({}) }) as DOMRect;

// jsdom has no layout: give the wrapper of the trigger and the tip the boxes a browser would measure.
const layout = (trigger: DOMRect, tip: DOMRect) =>
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    return this.getAttribute("role") === "tooltip" ? tip : this.querySelector("button") ? trigger : box(0, 0, 0, 0);
  });

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("Tooltip", () => {
  test("a label and a shortcut render inside the wrapper with no portal", () => {
    const { container } = render(<Tooltip label="Rare" shortcut="R" side="top" className="tip"><button>Rarity</button></Tooltip>);

    const tip = screen.getByRole("tooltip");
    expect(tip.textContent).toBe("Rare[R]");
    expect(container.contains(tip)).toBe(true);
    expect(tip.getAttribute("style")).toBeNull();
    expect(tip.id).toBe("");
  });

  test("a portal tip with a label opens on hover and closes when the pointer leaves", async () => {
    const user = userEvent.setup();
    render(<Tooltip label="Move" portal><button>Tool</button></Tooltip>);
    expect(screen.queryByRole("tooltip")).toBeNull();

    await user.hover(screen.getByRole("button", { name: "Tool" }));
    expect(screen.getByRole("tooltip").textContent).toBe("Move");

    await user.unhover(screen.getByRole("button", { name: "Tool" }));
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  test("rich content shows in place of the label", async () => {
    const user = userEvent.setup();
    render(<Tooltip portal content={<>Sets the size. <a href="https://example.test/docs">Learn more</a></>}><button>Size</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Size" }));

    expect(screen.getByRole("tooltip").textContent).toBe("Sets the size. Learn more");
    expect(screen.getByRole("link", { name: "Learn more" }).getAttribute("href")).toBe("https://example.test/docs");
  });

  test("an interactive tip stays open while the pointer moves into it", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(<Tooltip interactive content={<a href="https://example.test/docs">Learn more</a>}><button>Size</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Size" }));
    await user.hover(screen.getByRole("link", { name: "Learn more" }));
    act(() => { vi.advanceTimersByTime(1000); });

    expect(screen.getByRole("link", { name: "Learn more" })).toBeInTheDocument();
  });

  test("an interactive tip closes soon after the pointer leaves it", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(<Tooltip interactive content={<a href="https://example.test/docs">Learn more</a>}><button>Size</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Size" }));
    await user.hover(screen.getByRole("link", { name: "Learn more" }));
    await user.unhover(screen.getByRole("link", { name: "Learn more" }));
    act(() => { vi.advanceTimersByTime(1000); });

    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  test("a pointer press inside an interactive tip does not close it", async () => {
    const user = userEvent.setup();
    render(<Tooltip interactive content={<a href="#docs">Learn more</a>}><button>Size</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Size" }));
    await user.hover(screen.getByRole("link", { name: "Learn more" }));
    fireEvent.pointerDown(screen.getByRole("link", { name: "Learn more" }));

    expect(screen.getByRole("tooltip")).toBeInTheDocument();
  });

  test("a controlled tip follows the open prop and reports hover without opening itself", async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    const { rerender } = render(<Tooltip label="Door" open={false} onOpenChange={onOpenChange}><button>Asset</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Asset" }));
    expect(screen.queryByRole("tooltip")).toBeNull();
    expect(onOpenChange).toHaveBeenLastCalledWith(true);

    rerender(<Tooltip label="Door" open onOpenChange={onOpenChange}><button>Asset</button></Tooltip>);
    expect(screen.getByRole("tooltip").textContent).toBe("Door");

    await user.unhover(screen.getByRole("button", { name: "Asset" }));
    expect(screen.getByRole("tooltip")).toBeInTheDocument();
    expect(onOpenChange).toHaveBeenLastCalledWith(false);
    expect(onOpenChange).toHaveBeenCalledTimes(2);
  });

  test("a tip the parent opened reports the Escape key", async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    render(<Tooltip label="Door" open onOpenChange={onOpenChange}><button>Asset</button></Tooltip>);

    await user.keyboard("{Escape}");

    expect(onOpenChange.mock.calls).toEqual([[false]]);
  });

  test("an uncontrolled tip reports each change once", async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    render(<Tooltip label="Door" onOpenChange={onOpenChange}><button>Asset</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Asset" }));
    await user.keyboard("{Escape}");
    await user.unhover(screen.getByRole("button", { name: "Asset" }));

    expect(onOpenChange.mock.calls).toEqual([[true], [false]]);
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  test("the wrapper names the open tip as its description", async () => {
    const user = userEvent.setup();
    render(<Tooltip label="Move" portal><button>Tool</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Tool" }));

    const tip = screen.getByRole("tooltip");
    expect(tip.id).not.toBe("");
    expect(screen.getByRole("button", { name: "Tool" }).parentElement?.getAttribute("aria-describedby")).toBe(tip.id);
  });

  test("a tip on an outside anchor opens from the anchor and describes it", async () => {
    const user = userEvent.setup();
    const Anchored = () => {
      const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
      return <><button ref={setAnchor}>Tile</button><Tooltip label="Door" anchor={anchor} /></>;
    };
    render(<Anchored />);
    const tile = screen.getByRole("button", { name: "Tile" });

    await user.hover(tile);
    expect(screen.getByRole("tooltip").textContent).toBe("Door");
    expect(tile.getAttribute("aria-describedby")).toBe(screen.getByRole("tooltip").id);

    await user.unhover(tile);
    expect(screen.queryByRole("tooltip")).toBeNull();
    expect(tile.hasAttribute("aria-describedby")).toBe(false);
  });

  test("a controlled tip on an outside anchor sits beside the anchor", () => {
    const anchor = document.createElement("div");
    document.body.append(anchor);
    vi.spyOn(anchor, "getBoundingClientRect").mockReturnValue(box(100, 200, 80, 40));

    render(<Tooltip content={<p>Opens on click.</p>} anchor={anchor} open bare tipClassName="item-tip" />);

    const tip = screen.getByRole("tooltip");
    expect(tip.style.left).toBe("192px");
    expect(tip.style.top).toBe("220px");
    expect(tip.classList.contains("item-tip")).toBe(true);
    anchor.remove();
  });

  test("a maximum width reaches the tip as a custom property", () => {
    render(<Tooltip label="A long explanation of the field" maxWidth={250}><button>Field</button></Tooltip>);

    expect(screen.getByRole("tooltip").style.getPropertyValue("--tt-max-width")).toBe("250px");
  });

  test("a tip that does not fit on its side flips to the opposite side", async () => {
    const user = userEvent.setup();
    layout(box(window.innerWidth - 60, 300, 40, 20), box(0, 0, 200, 50));
    render(<Tooltip label="Wide" side="right" keepInViewport><button>Edge</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Edge" }));

    const tip = screen.getByRole("tooltip");
    expect(tip.style.left).toBe(window.innerWidth - 60 - 12 - 200 + "px");
    expect(tip.style.top).toBe("285px");
  });

  test("a tip that crosses a viewport edge shifts back inside", async () => {
    const user = userEvent.setup();
    layout(box(4, 300, 40, 20), box(0, 0, 200, 50));
    render(<Tooltip label="Wide" side="top" keepInViewport><button>Edge</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Edge" }));

    const tip = screen.getByRole("tooltip");
    expect(tip.style.left).toBe("8px");
    expect(tip.style.top).toBe("238px");
  });

  test("a tip that fits stays on its side", async () => {
    const user = userEvent.setup();
    layout(box(400, 300, 40, 20), box(0, 0, 200, 50));
    render(<Tooltip label="Wide" side="right" keepInViewport><button>Middle</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Middle" }));

    expect(screen.getByRole("tooltip").style.left).toBe("452px");
  });

  test("a tip set to close on scroll closes when the page scrolls", async () => {
    const user = userEvent.setup();
    render(<Tooltip label="Move" closeOnScroll><button>Tool</button></Tooltip>);
    await user.hover(screen.getByRole("button", { name: "Tool" }));

    fireEvent.scroll(window);

    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  test("a portal tip stays open when the page scrolls", async () => {
    const user = userEvent.setup();
    render(<Tooltip label="Move" portal><button>Tool</button></Tooltip>);
    await user.hover(screen.getByRole("button", { name: "Tool" }));

    fireEvent.scroll(window);

    expect(screen.getByRole("tooltip")).toBeInTheDocument();
  });

  test("a disabled tip never opens", async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    const { rerender } = render(<Tooltip label="Reload" portal disabled onOpenChange={onOpenChange}><button>Reload</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Reload" }));
    expect(screen.queryByRole("tooltip")).toBeNull();
    expect(onOpenChange).not.toHaveBeenCalled();

    rerender(<Tooltip label="Reload" disabled><button>Reload</button></Tooltip>);
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  test("a tip with an arrow draws its pointer, and a tip with none does not", () => {
    const { rerender } = render(<Tooltip label="Rare" side="top" arrow><button>Rarity</button></Tooltip>);
    expect(screen.getByRole("tooltip")).toHaveClass("tt__tip--arrow", "tt__tip--top");

    rerender(<Tooltip label="Rare" side="top"><button>Rarity</button></Tooltip>);
    expect(screen.getByRole("tooltip")).not.toHaveClass("tt__tip--arrow");
    expect(screen.getByRole("tooltip")).toHaveClass("tt__tip--top");
  });

  test("the arrow of a tip that flipped points from the side the tip is on", async () => {
    const user = userEvent.setup();
    layout(box(window.innerWidth - 60, 300, 40, 20), box(0, 0, 200, 50));
    render(<Tooltip label="Wide" side="right" keepInViewport arrow><button>Edge</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Edge" }));

    expect(screen.getByRole("tooltip")).toHaveClass("tt__tip--arrow", "tt__tip--left");
    expect(screen.getByRole("tooltip")).not.toHaveClass("tt__tip--right");
  });

  test("a tip with an enter delay opens when the pointer stayed that long", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    const onOpenChange = vi.fn();
    render(<Tooltip label="Move" enterDelay={2000} onOpenChange={onOpenChange}><button>Tool</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Tool" }));
    act(() => { vi.advanceTimersByTime(1000); });
    expect(screen.queryByRole("tooltip")).toBeNull();
    expect(onOpenChange).not.toHaveBeenCalled();

    act(() => { vi.advanceTimersByTime(1500); });
    expect(screen.getByRole("tooltip").textContent).toBe("Move");
    expect(onOpenChange.mock.calls).toEqual([[true]]);
  });

  test("a tip with an enter delay stays closed when the pointer leaves before that", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    const onOpenChange = vi.fn();
    render(<Tooltip label="Move" enterDelay={2000} onOpenChange={onOpenChange}><button>Tool</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Tool" }));
    act(() => { vi.advanceTimersByTime(1000); });
    await user.unhover(screen.getByRole("button", { name: "Tool" }));
    act(() => { vi.advanceTimersByTime(5000); });

    expect(screen.queryByRole("tooltip")).toBeNull();
    expect(onOpenChange).not.toHaveBeenCalled();
  });

  test("an interactive tip with an enter delay never opens when the pointer leaves before the delay", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    const onOpenChange = vi.fn();
    render(<Tooltip content={<a href="/docs">Docs</a>} interactive enterDelay={100} onOpenChange={onOpenChange}><button>Tool</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Tool" }));
    act(() => { vi.advanceTimersByTime(50); });
    await user.unhover(screen.getByRole("button", { name: "Tool" }));
    act(() => { vi.advanceTimersByTime(1000); });

    expect(screen.queryByRole("tooltip")).toBeNull();
    expect(onOpenChange).not.toHaveBeenCalled();
  });

  test("a controlled tip asks to open on the next hover after the parent refused a request while it closed", async () => {
    const user = userEvent.setup();
    const asked: boolean[] = [];
    function Host() {
      const [open, setOpen] = useState(true);
      const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
      // The parent moves focus back to the anchor when the tip closes, and refuses the open request that focus makes.
      const returning = useRef(false);
      const onOpenChange = (next: boolean) => {
        asked.push(next);
        if (returning.current) return;
        if (!next) {
          returning.current = true;
          anchor?.focus();
          returning.current = false;
        }
        setOpen(next);
      };
      return (
        <Tooltip label="Move" open={open} onOpenChange={onOpenChange} anchor={anchor}>
          <button ref={setAnchor}>Tool</button>
        </Tooltip>
      );
    }
    // The focus the parent moves is keyboard focus: the Escape key came before it.
    const matches = Element.prototype.matches;
    const keyboardFocus = vi.spyOn(Element.prototype, "matches").mockImplementation(function (this: Element, selector: string) {
      return selector === ":focus-visible" ? this === document.activeElement : matches.call(this, selector);
    });
    try {
      render(<Host />);
      expect(screen.getByRole("tooltip")).toBeInTheDocument();

      await user.keyboard("{Escape}");
      expect(asked).toEqual([false, true]);
      expect(screen.queryByRole("tooltip")).toBeNull();
      expect(screen.getByRole("button", { name: "Tool" })).toHaveFocus();

      asked.length = 0;
      await user.hover(screen.getByRole("button", { name: "Tool" }));

      expect(asked).toEqual([true]);
      expect(screen.getByRole("tooltip").textContent).toBe("Move");
    } finally {
      keyboardFocus.mockRestore();
    }
  });

  test("a tip with an enter delay opens at once on keyboard focus and closes when focus leaves", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(<><Tooltip label="Move" enterDelay={2000}><button>Tool</button></Tooltip><button>Next</button></>);

    await user.tab();
    expect(screen.getByRole("button", { name: "Tool" })).toHaveFocus();
    expect(screen.getByRole("tooltip").textContent).toBe("Move");

    await user.tab();
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  test("a tip with an enter delay closes on Escape", async () => {
    const user = userEvent.setup();
    render(<Tooltip label="Move" enterDelay={0}><button>Tool</button></Tooltip>);
    await user.hover(screen.getByRole("button", { name: "Tool" }));
    expect(screen.getByRole("tooltip")).toBeInTheDocument();

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  test("a tip that labels its child is the name of the control and not its description", () => {
    render(<Tooltip label="Delete" labelChild><button><svg aria-hidden="true" /></button></Tooltip>);

    const button = screen.getByRole("button", { name: "Delete" });
    expect(button).toHaveAccessibleDescription("");
    expect(button.parentElement?.hasAttribute("aria-describedby")).toBe(false);
    expect(screen.getByRole("tooltip").textContent).toBe("Delete");
  });

  test("a portal tip that labels its child names the control before it opens and does not describe it when open", async () => {
    const user = userEvent.setup();
    render(<Tooltip label="Delete" labelChild portal><button><svg aria-hidden="true" /></button></Tooltip>);
    const button = screen.getByRole("button", { name: "Delete" });
    expect(screen.queryByRole("tooltip")).toBeNull();

    await user.hover(button);

    expect(screen.getByRole("tooltip").textContent).toBe("Delete");
    expect(screen.getByRole("button", { name: "Delete" })).toBe(button);
    expect(button).toHaveAccessibleDescription("");
    expect(button.parentElement?.hasAttribute("aria-describedby")).toBe(false);
  });

  test("rich content that labels its child names the control while the tip is open", async () => {
    const user = userEvent.setup();
    render(<Tooltip content={<b>Delete the scene</b>} labelChild portal><button><svg aria-hidden="true" /></button></Tooltip>);
    const button = screen.getByRole("button");

    await user.hover(button);
    expect(screen.getByRole("button", { name: "Delete the scene" })).toBe(button);

    await user.unhover(button);
    expect(button.hasAttribute("aria-labelledby")).toBe(false);
  });

  test("a control that has a name keeps it", () => {
    render(<Tooltip label="Delete" labelChild><button aria-label="Remove the scene"><svg aria-hidden="true" /></button></Tooltip>);

    expect(screen.getByRole("button", { name: "Remove the scene" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Delete" })).toBeNull();
  });

  test("a tip that labels an outside anchor names the anchor and does not describe it", async () => {
    const user = userEvent.setup();
    const Anchored = () => {
      const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
      return <><button ref={setAnchor}><svg aria-hidden="true" /></button><Tooltip label="Door" anchor={anchor} labelChild /></>;
    };
    render(<Anchored />);
    const tile = screen.getByRole("button", { name: "Door" });

    await user.hover(tile);

    expect(screen.getByRole("tooltip").textContent).toBe("Door");
    expect(tile.hasAttribute("aria-describedby")).toBe(false);
  });

  test("a disabled tip that labels its child gives the control no name", () => {
    render(<Tooltip label="Delete" labelChild disabled><button><svg aria-hidden="true" /></button></Tooltip>);

    expect(screen.queryByRole("button", { name: "Delete" })).toBeNull();
    expect(screen.getByRole("button").hasAttribute("aria-label")).toBe(false);
  });

  test("a tip that does not label its child leaves the name of the control as it is", async () => {
    const user = userEvent.setup();
    render(<Tooltip label="Move" portal><button>Tool</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Tool" }));

    expect(screen.getByRole("button", { name: "Tool" }).hasAttribute("aria-label")).toBe(false);
    expect(screen.getByRole("button", { name: "Tool" }).hasAttribute("aria-labelledby")).toBe(false);
  });

  test.each([
    ["start", "bottom", "400px", "342px"],
    ["center", "bottom", "420px", "342px"],
    ["end", "bottom", "440px", "342px"],
    ["start", "top", "400px", "268px"],
    ["start", "right", "512px", "300px"],
    ["end", "right", "512px", "310px"],
    ["end", "left", "328px", "310px"],
  ] as const)("a tip aligned to the %s on the %s lines up with that edge of the anchor", async (align, side, left, top) => {
    const user = userEvent.setup();
    layout(box(400, 300, 100, 30), box(0, 0, 60, 20));
    render(<Tooltip label="Move" side={side} align={align} keepInViewport><button>Tool</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Tool" }));

    const tip = screen.getByRole("tooltip");
    expect(tip.style.left).toBe(left);
    expect(tip.style.top).toBe(top);
  });

  test("a tip with no alignment is centred on the anchor", async () => {
    const user = userEvent.setup();
    layout(box(400, 300, 100, 30), box(0, 0, 60, 20));
    render(<Tooltip label="Move" side="bottom" keepInViewport><button>Tool</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Tool" }));

    expect(screen.getByRole("tooltip").style.left).toBe("420px");
  });

  test("a snapped tip above its anchor has its lower edge on a whole device pixel", async () => {
    const user = userEvent.setup();
    layout(box(400.3, 300.4, 101, 30), box(0, 0, 60, 24.5));
    render(<Tooltip label="Move" side="top" keepInViewport snap><button>Tool</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Tool" }));

    const tip = screen.getByRole("tooltip");
    expect(tip.style.left).toBe("421px");
    expect(tip.style.top).toBe("263.5px");
  });

  test("a snapped tip sits on whole device pixels", async () => {
    const user = userEvent.setup();
    vi.stubGlobal("devicePixelRatio", 2);
    layout(box(400.3, 300.4, 101, 30), box(0, 0, 60.4, 20));
    render(<Tooltip label="Move" side="bottom" keepInViewport snap><button>Tool</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Tool" }));

    const tip = screen.getByRole("tooltip");
    expect(tip.style.left).toBe("420.5px");
    expect(tip.style.top).toBe("342.5px");
  });

  test("a tip that is not snapped keeps the measured position", async () => {
    const user = userEvent.setup();
    layout(box(400.3, 300.4, 101, 30), box(0, 0, 60.4, 20));
    render(<Tooltip label="Move" side="bottom" keepInViewport><button>Tool</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Tool" }));

    expect(screen.getByRole("tooltip").style.top).toBe("342.4px");
  });

  test("an aligned tip that crosses a viewport edge shifts back inside", async () => {
    const user = userEvent.setup();
    layout(box(window.innerWidth - 40, 300, 30, 30), box(0, 0, 200, 20));
    render(<Tooltip label="Move" side="bottom" align="start" keepInViewport><button>Tool</button></Tooltip>);

    await user.hover(screen.getByRole("button", { name: "Tool" }));

    expect(screen.getByRole("tooltip").style.left).toBe(window.innerWidth - 8 - 200 + "px");
  });
});
