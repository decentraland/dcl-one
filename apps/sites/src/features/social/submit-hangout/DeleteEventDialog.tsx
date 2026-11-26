import { Modal, Notice, useEventsCopy, Button } from "@dcl/one-ui";
import { useLayoutEffect, useRef, type MouseEvent } from "react";

/** Why the last delete did not happen: the request failed, or the signed-in account is no longer the editor's. */
export type DeleteEventError = "failed" | "session";

/**
 * Confirms deleting an event; it cannot be dismissed while the delete is in flight. Its buttons are
 * disabled meanwhile, so focus waits on the dialog itself instead of falling to the page, and a failed
 * delete hands it back to the Delete button for the retry.
 */
export default function DeleteEventDialog({ busy, error, onCancel, onConfirm }: {
  busy: boolean;
  error: DeleteEventError | null;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const { text } = useEventsCopy();
  /** Focus starts on Cancel, the action that keeps the event. */
  const cancelButton = useRef<HTMLButtonElement | null>(null);
  /** The Delete button that started the delete, set by its click. */
  const deleteButton = useRef<HTMLButtonElement | null>(null);
  const wasBusy = useRef(busy);
  // Only a delete that starts disables Delete event: focus waits on the dialog while it runs and comes back
  // when it ends without deleting. A delete refused before it starts never takes focus off the button.
  // Layout timing moves focus before the browser drops it from the disabled button.
  useLayoutEffect(() => {
    if (busy) deleteButton.current?.closest("dialog")?.focus();
    else if (wasBusy.current) deleteButton.current?.focus();
    wasBusy.current = busy;
  }, [busy]);
  const confirm = (event: MouseEvent<HTMLButtonElement>) => {
    deleteButton.current = event.currentTarget;
    onConfirm();
  };
  return (
    <Modal
      title={text("Are you sure you want to delete your event?")}
      description={text("This event will be permanently removed from your events. This action can’t be undone.")}
      onClose={onCancel}
      dismissible={!busy}
      closeButton={false}
      tabIndex={-1}
      initialFocusRef={cancelButton}
      // Every action is disabled meanwhile: Tab would move focus out of the dialog to the browser.
      onKeyDown={(event) => { if (busy && event.key === "Tab") event.preventDefault(); }}
      actions={<>
        <Button ref={cancelButton} size="lg" variant="inverse" disabled={busy} onClick={onCancel}>{text("Cancel")}</Button>
        <Button size="lg" disabled={busy} aria-busy={busy || undefined} onClick={confirm}>{busy ? text("Deleting…") : text("Delete event")}</Button>
      </>}
    >
      {error && <Notice tone="error">{text(error === "session" ? "Your session changed. Sign in again to delete this event." : "We couldn't delete your event. Please try again.")}</Notice>}
    </Modal>
  );
}
