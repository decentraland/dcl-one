import { Dialog, Notice, useEventsCopy, Button } from "@dcl/one-ui";
import { useEffect, useId, useRef, type MouseEvent } from "react";

import styles from "./DeleteEventDialog.module.css";

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
  const titleId = useId();
  /** The Delete button that started the delete, set by its click. */
  const deleteButton = useRef<HTMLButtonElement | null>(null);
  const wasBusy = useRef(busy);
  useEffect(() => {
    if (wasBusy.current && !busy) deleteButton.current?.focus();
    wasBusy.current = busy;
  }, [busy]);
  const cancel = () => { if (!busy) onCancel(); };
  const confirm = (event: MouseEvent<HTMLButtonElement>) => {
    deleteButton.current = event.currentTarget;
    event.currentTarget.closest<HTMLElement>("[role=dialog]")?.focus();
    onConfirm();
  };
  return (
    <Dialog labelledBy={titleId} onClose={cancel} maxWidth={540}>
      <div className={styles.body}>
        <h2 id={titleId} className={styles.title}>{text("Are you sure you want to delete your event?")}</h2>
        <p className={styles.copy}>{text("This event will be permanently removed from your events. This action can’t be undone.")}</p>
        {error && <Notice tone="error">{text(error === "session" ? "Your session changed. Sign in again to delete this event." : "We couldn't delete your event. Please try again.")}</Notice>}
        <div className={styles.actions}>
          <Button size="lg" variant="inverse" disabled={busy} onClick={cancel}>{text("Cancel")}</Button>
          <Button size="lg" disabled={busy} aria-busy={busy || undefined} onClick={confirm}>{busy ? text("Deleting…") : text("Delete event")}</Button>
        </div>
      </div>
    </Dialog>
  );
}
