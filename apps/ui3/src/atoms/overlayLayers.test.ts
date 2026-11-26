import { afterEach, describe, expect, test, vi } from "vitest";
import { pushLayer } from "./overlayLayers";

const removals: Array<() => void> = [];
afterEach(() => { while (removals.length) removals.pop()!(); });

function escape(init: KeyboardEventInit = {}) {
  const event = new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true, ...init });
  document.body.dispatchEvent(event);
  return event;
}

describe("overlay layers", () => {
  test("a modal top layer that does not close on Escape keeps the key from the page behind it", () => {
    const page = vi.fn();
    document.addEventListener("keydown", page);
    removals.push(() => document.removeEventListener("keydown", page));
    removals.push(pushLayer({ contains: () => false, modal: true }));

    const event = escape();

    expect(page).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(true);
  });

  test("a non-modal top layer with no Escape handler lets the key through", () => {
    const page = vi.fn();
    document.addEventListener("keydown", page);
    removals.push(() => document.removeEventListener("keydown", page));
    removals.push(pushLayer({ contains: () => false }));

    escape();

    expect(page).toHaveBeenCalledTimes(1);
  });

  test("an Escape that ends a text composition does not close the top layer", () => {
    const onEscape = vi.fn();
    removals.push(pushLayer({ contains: () => false, modal: true, onEscape }));

    const event = escape({ isComposing: true });

    expect(onEscape).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(false);

    escape();
    expect(onEscape).toHaveBeenCalledTimes(1);
  });
});
