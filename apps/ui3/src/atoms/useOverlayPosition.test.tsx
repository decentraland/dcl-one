import { useRef } from "react";
import { describe, expect, test } from "vitest";
import { render, screen } from "@testing-library/react";
import { useOverlayPosition } from "./useOverlayPosition";

const point = (x: number, y: number) => DOMRect.fromRect({ x, y, width: 0, height: 0 });

let renders = 0;

/** A context menu at a point: the anchor is a new object on every render. */
function ContextMenu({ x, y }: { x: number; y: number }) {
  renders++;
  const surface = useRef<HTMLDivElement>(null);
  const position = useOverlayPosition(true, { getBoundingClientRect: () => point(x, y) }, surface);
  return <div ref={surface} data-testid="menu" data-ready={position.ready} style={{ left: position.left, top: position.top }} />;
}

describe("useOverlayPosition", () => {
  test("an anchor that is a new object on each render is placed once and does not render again", () => {
    renders = 0;
    render(<ContextMenu x={100} y={120} />);

    const menu = screen.getByTestId("menu");
    expect(menu.dataset.ready).toBe("true");
    expect(menu.style.left).toBe("100px");
    expect(menu.style.top).toBe("120px");
    expect(renders).toBeLessThanOrEqual(3);
  });

  test("a new point for the anchor moves the surface", () => {
    const { rerender } = render(<ContextMenu x={100} y={120} />);
    rerender(<ContextMenu x={200} y={220} />);

    const menu = screen.getByTestId("menu");
    expect(menu.style.left).toBe("200px");
    expect(menu.style.top).toBe("220px");
  });
});
