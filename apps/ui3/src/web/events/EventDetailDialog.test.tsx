import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";

import EventDetailDialog, { type EventDialogData } from "./EventDetailDialog";

const EVENT: EventDialogData = {
  id: "e1", name: "Party", description: "Music", image: null, x: 1, y: 2, creatorName: "Decentraland Foundation",
  startAt: "2026-11-10T20:00:00.000Z", finishAt: "2026-11-10T21:00:00.000Z", recurrent: false, recurrentFrequency: null,
  recurrentInterval: null, live: false, isWorld: false, placeName: null,
};

it("keeps Tab inside the dialog and gives focus back to the opener on close", async () => {
  const opener = document.createElement("button");
  document.body.append(opener);
  opener.focus();
  const view = render(
    <QueryClientProvider client={new QueryClient()}>
      <EventDetailDialog event={EVENT} signedIn={false} jumpHref="/jump" shareUrl="https://example.test/e1" onClose={vi.fn()} />
    </QueryClientProvider>,
  );
  const dialog = screen.getByRole("dialog", { name: "Party" });
  expect(dialog).toHaveFocus();
  const user = userEvent.setup();
  const focusable = [...dialog.querySelectorAll<HTMLElement>("a[href], button:not([disabled])")];
  await user.tab();
  expect(focusable[0]).toHaveFocus();
  focusable.at(-1)!.focus();
  await user.tab();
  expect(focusable[0]).toHaveFocus();
  await user.tab({ shift: true });
  expect(focusable.at(-1)).toHaveFocus();
  expect(dialog.querySelector("[tabindex='0']")).toBeNull();
  view.unmount();
  expect(opener).toHaveFocus();
  opener.remove();
});
