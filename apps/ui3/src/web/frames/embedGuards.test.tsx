import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import Modal from "../../components/Modal";
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

it("keeps Escape inside a hub framed by the Explorer, where it closes the hub's own dialog", async () => {
  uninstall = installEmbedGuards(window, { scope: ["/create"], open: vi.fn() });
  const onClose = vi.fn();
  render(<Modal onClose={onClose} ariaLabel="Delete project"><p>Delete it?</p></Modal>);
  await act(async () => {});
  expect(screen.getByRole("dialog", { name: "Delete project" })).toBeInTheDocument();
  fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
  expect(onClose).toHaveBeenCalledOnce();
  expect(parent.postMessage).not.toHaveBeenCalled();
});
