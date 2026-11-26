import { useRef } from "react";
import { describe, expect, test, vi } from "vitest";
import { fireEvent, render } from "@testing-library/react";
import { useDismiss } from "./useDismiss";

function Layer({ open, onEscape, modal = false, children }: { open: boolean; onEscape?: () => void; modal?: boolean; children?: React.ReactNode }) {
  const surface = useRef<HTMLDivElement>(null);
  useDismiss(open, { surface, onEscape, modal });
  return <div ref={surface}>{children}</div>;
}

describe("useDismiss", () => {
  test("a layer keeps its place under a layer opened after it when its options change", () => {
    const dialogEscape = vi.fn();
    const selectEscape = vi.fn();
    // The dialog's Escape handler changes identity and, in between, is missing (a dialog that is busy).
    const view = (busy: boolean, selectOpen = true) => (
      <Layer open modal onEscape={busy ? undefined : () => dialogEscape()}>
        <Layer open={selectOpen} onEscape={selectEscape} />
      </Layer>
    );
    const { rerender } = render(view(false, false));
    rerender(view(false));
    rerender(view(true));
    rerender(view(false));

    fireEvent.keyDown(document.body, { key: "Escape" });

    expect(selectEscape).toHaveBeenCalledTimes(1);
    expect(dialogEscape).not.toHaveBeenCalled();
  });

  test("a layer reads the Escape handler of the last render", () => {
    const first = vi.fn();
    const second = vi.fn();
    const { rerender } = render(<Layer open onEscape={first} />);
    rerender(<Layer open onEscape={second} />);

    fireEvent.keyDown(document.body, { key: "Escape" });

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledTimes(1);
  });
});
