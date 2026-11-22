// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { EMBED_ESCAPE_MESSAGE, installEmbedGuards } from "./embed";

let parent: { postMessage: ReturnType<typeof vi.fn> };
let uninstall: (() => void) | undefined;
const ownParent = Object.getOwnPropertyDescriptor(window, "parent");

beforeEach(() => {
  parent = { postMessage: vi.fn() };
  Object.defineProperty(window, "parent", { configurable: true, get: () => parent });
});
afterEach(() => {
  uninstall?.();
  uninstall = undefined;
  cleanup();
  if (ownParent) Object.defineProperty(window, "parent", ownParent);
  else Reflect.deleteProperty(window, "parent");
});

it("closes the Shop's frame on an Escape its page left alone", () => {
  uninstall = installEmbedGuards(window);
  fireEvent.keyDown(document.body, { key: "Escape" });
  expect(parent.postMessage).toHaveBeenCalledWith(EMBED_ESCAPE_MESSAGE, "*");
});

it("keeps Escape inside a hub framed by the Explorer, where it closes the hub's own dialog", () => {
  uninstall = installEmbedGuards(window, { scope: ["/create"], open: vi.fn() });
  const onClose = vi.fn();
  // The dialog leaves the event unconsumed, so only the missing relay keeps it from reaching the parent.
  render(
    <div role="dialog" aria-label="Delete project" tabIndex={-1} onKeyDown={(e) => e.key === "Escape" && onClose()}>
      <p>Delete it?</p>
    </div>,
  );
  fireEvent.keyDown(screen.getByRole("dialog", { name: "Delete project" }), { key: "Escape" });
  expect(onClose).toHaveBeenCalledOnce();
  expect(parent.postMessage).not.toHaveBeenCalled();
});
