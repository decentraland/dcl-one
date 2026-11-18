import { useRef, useState } from "react";
import { afterEach, describe, expect, test, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import Popover, { type PopoverCloseReason, type PopoverProps } from "./Popover";
import { useDismiss } from "./useDismiss";
import { computeOverlayPosition } from "./useOverlayPosition";

const box = (left: number, top: number, width: number, height: number) =>
  ({ left, top, width, height, right: left + width, bottom: top + height, x: left, y: top, toJSON: () => ({}) }) as DOMRect;

// jsdom has no layout: give the trigger and the surface the boxes a browser would measure.
const layout = (trigger: DOMRect, surface: DOMRect) =>
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    return this.getAttribute("role") === "dialog" ? surface : this.textContent === "Filters" ? trigger : box(0, 0, 0, 0);
  });

afterEach(() => {
  vi.restoreAllMocks();
});

type HarnessProps = Partial<PopoverProps> & { onClose?: (reason: PopoverCloseReason) => void; startOpen?: boolean };

function Harness({ onClose, startOpen = false, ...props }: HarnessProps) {
  const [open, setOpen] = useState(startOpen);
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  return (
    <>
      <button ref={setAnchor} onClick={() => setOpen((value) => !value)}>Filters</button>
      <button>Elsewhere</button>
      <p>Page text</p>
      <Popover
        open={open} anchor={anchor} role="dialog" aria-label="Filter options"
        onClose={(reason) => { onClose?.(reason); setOpen(false); }}
        {...props}
      >
        <button>Apply</button>
      </Popover>
    </>
  );
}

const surface = () => screen.getByRole("dialog", { name: "Filter options" });
const layerUnder = () => surface().previousElementSibling as HTMLElement;
/** The backdrop of a modal popover is the element before its surface. */
const backdrop = () => surface().previousElementSibling as HTMLElement;
const left = (node: HTMLElement) => node.style.getPropertyValue("--popover-left");
const top = (node: HTMLElement) => node.style.getPropertyValue("--popover-top");

describe("Popover", () => {
  test("a closed popover renders nothing", () => {
    render(<Harness />);

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.queryByRole("button", { name: "Apply" })).toBeNull();
  });

  test("an open popover has no role of its own and sits outside the tree that rendered it", () => {
    const { container } = render(<Harness startOpen role={undefined} aria-label={undefined} />);

    const content = screen.getByRole("button", { name: "Apply" });
    expect(content.parentElement?.hasAttribute("role")).toBe(false);
    expect(container.contains(content)).toBe(false);
    expect(document.body.contains(content)).toBe(true);
  });

  test("the role and the name of the caller reach the surface", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.click(screen.getByRole("button", { name: "Filters" }));

    expect(surface()).toContainElement(screen.getByRole("button", { name: "Apply" }));
  });

  test("Escape closes the popover and focus returns to the opener", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} autoFocus />);
    await user.click(screen.getByRole("button", { name: "Filters" }));
    expect(surface()).toHaveFocus();

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("escape");
    expect(screen.getByRole("button", { name: "Filters" })).toHaveFocus();
  });

  test("focus inside the popover returns to the opener when it closes", async () => {
    const user = userEvent.setup();
    render(<Harness autoFocus />);
    await user.click(screen.getByRole("button", { name: "Filters" }));
    await user.tab();
    expect(screen.getByRole("button", { name: "Apply" })).toHaveFocus();

    await user.keyboard("{Escape}");

    expect(screen.getByRole("button", { name: "Filters" })).toHaveFocus();
  });

  test("a popover with no autoFocus leaves focus on the opener", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.click(screen.getByRole("button", { name: "Filters" }));

    expect(surface()).not.toHaveAttribute("tabindex");
    expect(screen.getByRole("button", { name: "Filters" })).toHaveFocus();
  });

  test("a press outside closes the popover", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />);
    await user.click(screen.getByRole("button", { name: "Filters" }));

    await user.click(screen.getByText("Page text"));

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("outside");
  });

  test("a press inside the popover keeps it open", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />);
    await user.click(screen.getByRole("button", { name: "Filters" }));

    await user.click(screen.getByRole("button", { name: "Apply" }));

    expect(surface()).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  test("a press on the opener is not an outside press: the opener closes the popover itself", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />);
    await user.click(screen.getByRole("button", { name: "Filters" }));

    await user.click(screen.getByRole("button", { name: "Filters" }));

    expect(onClose).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  test("a press on another control closes the popover and focus stays on that control", async () => {
    const user = userEvent.setup();
    render(<Harness autoFocus />);
    await user.click(screen.getByRole("button", { name: "Filters" }));

    await user.click(screen.getByRole("button", { name: "Elsewhere" }));

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByRole("button", { name: "Elsewhere" })).toHaveFocus();
  });

  test("a popover that does not close on Escape or on an outside press stays open", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} closeOnEscape={false} closeOnOutside={false} />);
    await user.click(screen.getByRole("button", { name: "Filters" }));

    await user.keyboard("{Escape}");
    await user.click(screen.getByText("Page text"));

    expect(surface()).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  test("a popover that is not modal has no backdrop", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const before = document.body.childElementCount;

    await user.click(screen.getByRole("button", { name: "Filters" }));

    expect(document.body.childElementCount).toBe(before + 1);
    expect(surface().parentElement).toBe(document.body);
  });

  test("a press on the backdrop of a modal popover closes it", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Harness modal onClose={onClose} />);
    await user.click(screen.getByRole("button", { name: "Filters" }));
    expect(backdrop()).toHaveAttribute("aria-hidden", "true");

    await user.click(backdrop());

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("outside");
  });

  test("a press on the backdrop is not a press on the page: no page handler runs and focus stays on the opener", async () => {
    const user = userEvent.setup();
    const onPagePress = vi.fn();
    render(<div onClick={onPagePress}><Harness modal /></div>);
    await user.click(screen.getByRole("button", { name: "Filters" }));
    onPagePress.mockClear();

    await user.click(backdrop());

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onPagePress).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Filters" })).toHaveFocus();
  });

  test("focus inside a modal popover returns to the opener after a press on the backdrop", async () => {
    const user = userEvent.setup();
    render(<Harness modal autoFocus />);
    await user.click(screen.getByRole("button", { name: "Filters" }));
    expect(surface()).toHaveFocus();

    await user.click(backdrop());

    expect(screen.getByRole("button", { name: "Filters" })).toHaveFocus();
  });

  test("a press inside a modal popover keeps it open", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Harness modal onClose={onClose} />);
    await user.click(screen.getByRole("button", { name: "Filters" }));

    await user.click(screen.getByRole("button", { name: "Apply" }));

    expect(surface()).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  test("a modal popover that does not close on an outside press stays open on a backdrop press", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Harness modal closeOnOutside={false} onClose={onClose} />);
    await user.click(screen.getByRole("button", { name: "Filters" }));

    await user.click(backdrop());

    expect(surface()).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  test("Escape closes a modal popover", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Harness modal onClose={onClose} />);
    await user.click(screen.getByRole("button", { name: "Filters" }));

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onClose).toHaveBeenCalledWith("escape");
  });

  test("the page behind does not scroll while a popover that locks the scroll is open", async () => {
    const user = userEvent.setup();
    render(<Harness lockScroll />);
    await user.click(screen.getByRole("button", { name: "Filters" }));
    expect(document.body.style.overflow).toBe("hidden");

    await user.keyboard("{Escape}");

    expect(document.body.style.overflow).toBe("");
  });

  test("a modal popover that is asked not to lock the scroll leaves the page scroll as it is", async () => {
    const user = userEvent.setup();
    render(<Harness modal lockScroll={false} />);

    await user.click(screen.getByRole("button", { name: "Filters" }));

    expect(document.body.style.overflow).toBe("");
  });

  test("a popover that does not return focus leaves it on the page", async () => {
    const user = userEvent.setup();
    render(<Harness autoFocus returnFocus={false} />);
    await user.click(screen.getByRole("button", { name: "Filters" }));

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByRole("button", { name: "Filters" })).not.toHaveFocus();
  });

  test("a modal popover has a layer under it, and a press on that layer closes the popover", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} modal />);
    await user.click(screen.getByRole("button", { name: "Filters" }));

    await user.click(layerUnder());

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledWith("outside");
  });

  test("a popover that is not modal has no layer under it", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Filters" }));

    expect(surface().parentElement).toBe(document.body);
    expect(layerUnder().getAttribute("aria-hidden")).toBeNull();
  });

  test("a press inside a modal popover keeps it open", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} modal />);
    await user.click(screen.getByRole("button", { name: "Filters" }));

    await user.click(screen.getByRole("button", { name: "Apply" }));

    expect(surface()).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  test("a modal popover that does not close on an outside press stays open", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} modal closeOnOutside={false} />);
    await user.click(screen.getByRole("button", { name: "Filters" }));

    await user.click(layerUnder());

    expect(surface()).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  test("Escape closes a modal popover", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} modal autoFocus />);
    await user.click(screen.getByRole("button", { name: "Filters" }));

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onClose).toHaveBeenCalledWith("escape");
  });

  test("the page behind does not scroll while a modal popover is open", async () => {
    const user = userEvent.setup();
    render(<Harness modal />);
    expect(document.body.style.overflow).toBe("");

    await user.click(screen.getByRole("button", { name: "Filters" }));
    expect(document.body.style.overflow).toBe("hidden");

    await user.click(layerUnder());
    expect(document.body.style.overflow).toBe("");
  });

  test("a popover that is not modal leaves the page free to scroll", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.click(screen.getByRole("button", { name: "Filters" }));

    expect(document.body.style.overflow).toBe("");
  });

  test("two modal popovers that close at the same time leave the page free to scroll", async () => {
    const user = userEvent.setup();
    function Pair() {
      const [open, setOpen] = useState(false);
      const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
      return (
        <>
          <button ref={setAnchor} onClick={() => setOpen(true)}>Open both</button>
          <Popover open={open} anchor={anchor} modal role="dialog" aria-label="First" onClose={() => setOpen(false)} />
          <Popover open={open} anchor={anchor} modal role="dialog" aria-label="Second" onClose={() => setOpen(false)} />
        </>
      );
    }
    render(<Pair />);
    await user.click(screen.getByRole("button", { name: "Open both" }));
    expect(document.body.style.overflow).toBe("hidden");

    await user.click(screen.getByRole("dialog", { name: "Second" }).previousElementSibling as HTMLElement);

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.body.style.overflow).toBe("");
  });

  test("the surface opens under the start edge of the anchor", async () => {
    const user = userEvent.setup();
    layout(box(100, 100, 80, 30), box(0, 0, 200, 100));
    render(<Harness />);

    await user.click(screen.getByRole("button", { name: "Filters" }));

    expect(left(surface())).toBe("100px");
    expect(top(surface())).toBe("138px");
    expect(surface()).toHaveAttribute("data-placement", "bottom-start");
  });

  test("a surface with no room under the anchor opens over it", async () => {
    const user = userEvent.setup();
    layout(box(100, 700, 80, 30), box(0, 0, 200, 100));
    render(<Harness />);

    await user.click(screen.getByRole("button", { name: "Filters" }));

    expect(top(surface())).toBe("592px");
    expect(surface()).toHaveAttribute("data-placement", "top-start");
  });

  test("a surface that crosses the viewport edge shifts back inside", async () => {
    const user = userEvent.setup();
    layout(box(950, 100, 60, 30), box(0, 0, 200, 100));
    render(<Harness />);

    await user.click(screen.getByRole("button", { name: "Filters" }));

    expect(left(surface())).toBe(window.innerWidth - 8 - 200 + "px");
  });

  test("the surface follows the anchor when the window changes size", async () => {
    const user = userEvent.setup();
    const boxes = layout(box(100, 100, 80, 30), box(0, 0, 200, 100));
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Filters" }));

    boxes.mockRestore();
    layout(box(300, 200, 80, 30), box(0, 0, 200, 100));
    fireEvent(window, new Event("resize"));

    expect(left(surface())).toBe("300px");
    expect(top(surface())).toBe("238px");
  });

  test("a surface that matches the anchor is at least as wide as it", async () => {
    const user = userEvent.setup();
    layout(box(100, 100, 240, 30), box(0, 0, 200, 100));
    render(<Harness matchAnchorWidth />);

    await user.click(screen.getByRole("button", { name: "Filters" }));

    expect(surface().style.getPropertyValue("--popover-min-width")).toBe("240px");
  });
});

describe("computeOverlayPosition", () => {
  const anchor = box(400, 300, 100, 40);
  const size = { width: 200, height: 80 };
  const viewport = { width: 1000, height: 800 };

  test("each side places the surface next to that edge of the anchor, centred on it", () => {
    expect(computeOverlayPosition(anchor, size, viewport, { placement: "bottom" })).toEqual({ left: 350, top: 340, placement: "bottom" });
    expect(computeOverlayPosition(anchor, size, viewport, { placement: "top" })).toEqual({ left: 350, top: 220, placement: "top" });
    expect(computeOverlayPosition(anchor, size, viewport, { placement: "right" })).toEqual({ left: 500, top: 280, placement: "right" });
    expect(computeOverlayPosition(anchor, size, viewport, { placement: "left" })).toEqual({ left: 200, top: 280, placement: "left" });
  });

  test("start and end line the surface up with an edge of the anchor", () => {
    expect(computeOverlayPosition(anchor, size, viewport, { placement: "bottom-start" }).left).toBe(400);
    expect(computeOverlayPosition(anchor, size, viewport, { placement: "bottom-end" }).left).toBe(300);
    expect(computeOverlayPosition(anchor, size, viewport, { placement: "right-start" }).top).toBe(300);
    expect(computeOverlayPosition(anchor, size, viewport, { placement: "right-end" }).top).toBe(260);
  });

  test("the offset moves the surface away from the anchor and the cross offset moves it along the edge", () => {
    expect(computeOverlayPosition(anchor, size, viewport, { placement: "bottom-start", offset: 8, crossOffset: 12 }))
      .toEqual({ left: 412, top: 348, placement: "bottom-start" });
    expect(computeOverlayPosition(anchor, size, viewport, { placement: "left-start", offset: 8, crossOffset: 12 }))
      .toEqual({ left: 192, top: 312, placement: "left-start" });
  });

  test("the default placement is under the start edge", () => {
    expect(computeOverlayPosition(anchor, size, viewport)).toEqual({ left: 400, top: 340, placement: "bottom-start" });
  });

  test("a surface that does not fit flips to the opposite side and keeps its alignment", () => {
    const low = box(400, 740, 100, 40);
    expect(computeOverlayPosition(low, size, viewport, { placement: "bottom-end" })).toEqual({ left: 300, top: 660, placement: "top-end" });
    const far = box(880, 300, 100, 40);
    expect(computeOverlayPosition(far, size, viewport, { placement: "right" })).toEqual({ left: 680, top: 280, placement: "left" });
  });

  test("a surface that fits on neither side stays on the asked side, inside the viewport", () => {
    const tall = { width: 200, height: 780 };
    expect(computeOverlayPosition(anchor, tall, viewport, { placement: "bottom" })).toEqual({ left: 350, top: 12, placement: "bottom" });
  });

  test("with no flip the surface stays on the asked side and is kept inside the viewport", () => {
    const low = box(400, 740, 100, 40);
    expect(computeOverlayPosition(low, size, viewport, { placement: "bottom", flip: false })).toEqual({ left: 350, top: 712, placement: "bottom" });
  });

  test("the edge distance keeps the surface off the viewport border", () => {
    const corner = box(0, 0, 20, 20);
    expect(computeOverlayPosition(corner, size, viewport, { placement: "bottom-end", edge: 16 }).left).toBe(16);
    const end = box(980, 300, 20, 20);
    expect(computeOverlayPosition(end, size, viewport, { placement: "bottom-start", edge: 16 }).left).toBe(784);
  });
});

type LayerProps = { name: string; open: boolean; modal?: boolean; escape?: boolean; onEscape: (name: string) => void; onOutside: (name: string) => void };

function Layer({ name, open, modal, escape = true, onEscape, onOutside }: LayerProps) {
  const ref = useRef<HTMLDivElement | null>(null);
  useDismiss(open, { surface: ref, modal, onEscape: escape ? () => onEscape(name) : undefined, onOutside: () => onOutside(name) });
  return open ? <div ref={ref} role="group" aria-label={name}><button>{name} control</button></div> : null;
}

describe("useDismiss", () => {
  function setup(upper: Partial<LayerProps> = {}, lower: Partial<LayerProps> = {}) {
    const onEscape = vi.fn();
    const onOutside = vi.fn();
    const tree = (lowerOpen: boolean, upperOpen: boolean) => (
      <>
        <p>Page text</p>
        <Layer name="Lower" open={lowerOpen} onEscape={onEscape} onOutside={onOutside} {...lower} />
        <Layer name="Upper" open={upperOpen} onEscape={onEscape} onOutside={onOutside} {...upper} />
      </>
    );
    const view = render(tree(true, false));
    view.rerender(tree(true, true));
    return { onEscape, onOutside, close: () => view.rerender(tree(true, false)) };
  }

  test("Escape reaches only the layer that opened last", async () => {
    const user = userEvent.setup();
    const { onEscape, close } = setup();

    await user.keyboard("{Escape}");
    expect(onEscape.mock.calls).toEqual([["Upper"]]);

    close();
    await user.keyboard("{Escape}");
    expect(onEscape.mock.calls).toEqual([["Upper"], ["Lower"]]);
  });

  test("a top layer that does not close on Escape keeps the key from the layers under it", async () => {
    const user = userEvent.setup();
    const { onEscape } = setup({ escape: false });

    await user.keyboard("{Escape}");

    expect(onEscape).not.toHaveBeenCalled();
  });

  test("Escape that a layer takes does not reach other listeners on the page", async () => {
    const user = userEvent.setup();
    const onKeyDown = vi.fn();
    document.addEventListener("keydown", onKeyDown);
    const { close } = setup();

    await user.keyboard("{Escape}");
    expect(onKeyDown).not.toHaveBeenCalled();

    close();
    document.removeEventListener("keydown", onKeyDown);
  });

  test("a press on the page is outside every open layer", async () => {
    const user = userEvent.setup();
    const { onOutside } = setup();

    await user.click(screen.getByText("Page text"));

    expect(onOutside.mock.calls).toEqual([["Upper"], ["Lower"]]);
  });

  test("a press inside a layer is outside only the layers over it", async () => {
    const user = userEvent.setup();
    const { onOutside } = setup();

    await user.click(screen.getByRole("button", { name: "Lower control" }));
    expect(onOutside.mock.calls).toEqual([["Upper"]]);

    await user.click(screen.getByRole("button", { name: "Upper control" }));
    expect(onOutside.mock.calls).toEqual([["Upper"]]);
  });

  test("a modal layer keeps a press on the page from the layers under it", async () => {
    const user = userEvent.setup();
    const { onOutside } = setup({ modal: true });

    await user.click(screen.getByText("Page text"));

    expect(onOutside).not.toHaveBeenCalled();
  });

  test("a closed layer gets no key and no press", async () => {
    const user = userEvent.setup();
    const onEscape = vi.fn();
    const onOutside = vi.fn();
    render(<><p>Page text</p><Layer name="Only" open={false} onEscape={onEscape} onOutside={onOutside} /></>);

    await user.keyboard("{Escape}");
    await user.click(screen.getByText("Page text"));

    expect(onEscape).not.toHaveBeenCalled();
    expect(onOutside).not.toHaveBeenCalled();
  });
});
