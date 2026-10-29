import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";

import Modal from "./Modal";

it("a dialog that closes on Escape claims it while open; one that does not leaves it to the HUD", async () => {
  const onClose = vi.fn();
  const { rerender, unmount } = render(<Modal ariaLabel="Closable" onClose={onClose}>body</Modal>);
  const dialog = screen.getByRole("dialog", { name: "Closable" });
  expect(dialog).toHaveAttribute("data-owns-escape");
  await userEvent.setup().keyboard("{Escape}");
  expect(onClose).toHaveBeenCalledTimes(1);

  rerender(<Modal ariaLabel="Closable" onClose={onClose} closeOnEsc={false}>body</Modal>);
  expect(screen.getByRole("dialog", { name: "Closable" })).not.toHaveAttribute("data-owns-escape");
  unmount();
});
