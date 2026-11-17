import { useEffect, useId, useRef, type RefObject } from "react";

import { Button } from "@dcl/one-ui";
import { GeneralClose } from "@dcl/one-ui/foundations/icons";

import { CREDITS_COPY } from "./copy";
import styles from "./LeaveCreditsDialog.module.css";

const COPY = CREDITS_COPY;

type LeaveCreditsDialogProps = {
  /** While the opt-out is in flight the dialog cannot be dismissed. */
  pending: boolean;
  error: string | undefined;
  onConfirm: () => void;
  onClose: () => void;
  /** Where focus goes back to on close; Safari does not focus a clicked button, so it is passed, not read. */
  returnFocusRef: RefObject<HTMLElement | null>;
};

/**
 * The confirmation before leaving the credits program, as decentraland.org's opt-out modal: a native
 * modal dialog over a backdrop, so focus stays inside and the page behind is inert. Escape, Close,
 * Cancel and a click on the backdrop dismiss it, except while leaving is in flight; the page does not
 * scroll behind it, and focus goes back to Leave Program. The design system has no Dialog yet.
 */
export function LeaveCreditsDialog({ pending, error, onConfirm, onClose, returnFocusRef }: LeaveCreditsDialogProps) {
  const ref = useRef<HTMLDialogElement>(null);
  const pressedOutside = useRef(false);
  const titleId = useId();
  const descriptionId = useId();
  const dismiss = () => {
    if (!pending) onClose();
  };

  useEffect(() => {
    const dialog = ref.current;
    const returnTo = returnFocusRef.current;
    const root = document.documentElement;
    const { overflow, paddingRight } = root.style;
    // The scrollbar's width becomes padding, so hiding it does not shift the page.
    const scrollbar = window.innerWidth - root.clientWidth;
    root.style.overflow = "hidden";
    if (scrollbar > 0) root.style.paddingRight = `${scrollbar}px`;
    dialog?.showModal();
    return () => {
      if (dialog?.open) dialog.close();
      root.style.overflow = overflow;
      root.style.paddingRight = paddingRight;
      if (returnTo?.isConnected) returnTo.focus();
    };
  }, [returnFocusRef]);

  /** Within the box, or on the backdrop around it. */
  const outside = (dialog: HTMLDialogElement, x: number, y: number) => {
    const box = dialog.getBoundingClientRect();
    return x < box.left || x > box.right || y < box.top || y > box.bottom;
  };

  return (
    <dialog
      ref={ref}
      className={styles.dialog}
      aria-labelledby={titleId}
      aria-describedby={descriptionId}
      onKeyDown={(event) => {
        if (event.key !== "Escape" || event.defaultPrevented) return;
        event.preventDefault();
        dismiss();
      }}
      onCancel={(event) => {
        event.preventDefault();
        dismiss();
      }}
      // A close the browser forces (a repeated Escape, a back gesture) cannot be refused: reopen while
      // leaving is in flight, otherwise let the page follow.
      onClose={() => {
        const dialog = ref.current;
        // The event arrives a task later: a dialog already shown again (a remount) has nothing to follow.
        if (!dialog || dialog.open) return;
        if (pending) dialog.showModal();
        else onClose();
      }}
      onPointerDown={(event) => {
        pressedOutside.current =
          event.target === event.currentTarget && outside(event.currentTarget, event.clientX, event.clientY);
      }}
      onClick={(event) => {
        // Only a press and release both on the backdrop: a drag out of the dialog does not close it.
        const fromOutside = pressedOutside.current;
        pressedOutside.current = false;
        if (event.target !== event.currentTarget || !fromOutside) return;
        if (outside(event.currentTarget, event.clientX, event.clientY)) dismiss();
      }}
    >
      <div className={styles.header}>
        <h2 id={titleId} className={styles.title}>
          {COPY.confirmTitle}
        </h2>
        <button type="button" className={styles.close} aria-label={COPY.close} onClick={dismiss} disabled={pending}>
          <GeneralClose size={20} aria-hidden="true" />
        </button>
      </div>
      <p id={descriptionId} className={styles.description}>
        {COPY.confirmDescription}
      </p>
      {error ? (
        <p className={styles.error} role="alert">
          {error}
        </p>
      ) : null}
      <div className={styles.actions}>
        <Button variant="secondary" size="md" onClick={dismiss} disabled={pending}>
          {COPY.cancel}
        </Button>
        <Button size="md" onClick={onConfirm} loading={pending}>
          {COPY.confirm}
        </Button>
      </div>
    </dialog>
  );
}
