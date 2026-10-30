import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import WearableDraftActions from "./WearableDraftActions";

it("requires confirmation, retains errors and prevents duplicate pending deletions", async () => {
  let finish!: () => void;
  const onDelete = vi.fn().mockRejectedValueOnce(new Error("Collection is publishing"))
    .mockImplementationOnce(() => new Promise<void>(resolve => { finish = resolve; }));
  render(<WearableDraftActions name="Jacket" onDelete={onDelete} />);
  const open = () => { fireEvent.click(screen.getByRole("button", { name: "Draft actions for Jacket" })); fireEvent.click(screen.getByRole("menuitem", { name: "Delete draft" })); };
  open();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(onDelete).not.toHaveBeenCalled();
  open();
  fireEvent.click(screen.getByRole("button", { name: "Delete draft" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Collection is publishing");
  fireEvent.click(screen.getByRole("button", { name: "Delete draft" }));
  const pending = screen.getByRole("button", { name: "Deleting…" });
  expect(pending).toBeDisabled();
  fireEvent.click(pending);
  expect(onDelete).toHaveBeenCalledTimes(2);
  finish();
  await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
});
