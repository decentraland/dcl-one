import { Notice } from "@dcl/one-ui/apps/sites/events/Notice";
import { useEventsCopy } from "@dcl/one-ui/apps/sites/events/locale";
import { Button } from "@dcl/one-ui/apps/sites/shared/Button/Button";
import { GeneralCheck } from "@dcl/one-ui";
import { useEffect, useRef } from "react";
import { useNavigate } from "react-router";

import styles from "./SubmitSuccess.module.css";

/**
 * The confirmation that covers the page below the site header once an event is created or saved.
 * The form it replaces held focus, so focus moves to the message instead of falling to the page.
 * An edit the server answers as still rejected says so instead of claiming success.
 */
export default function SubmitSuccess({ mode, approved, stillRejected = false }: {
  mode: "create" | "edit";
  approved: boolean;
  stillRejected?: boolean;
}) {
  const { text } = useEventsCopy();
  const navigate = useNavigate();
  const message = useRef<HTMLParagraphElement>(null);
  const notice = useRef<HTMLDivElement>(null);
  const held = mode === "edit" && stillRejected;
  useEffect(() => {
    const overflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    (notice.current ?? message.current)?.focus();
    return () => { document.body.style.overflow = overflow; };
  }, []);
  return (
    <section className={styles.overlay}>
      <div className={styles.content}>
        {held ? (
          <div ref={notice} className={styles.notice} tabIndex={-1}>
            <Notice tone="warning">{text("Your changes were saved, but the event is still rejected, so it is not back in review.")}</Notice>
          </div>
        ) : (
          <>
            <span className={styles.check}><GeneralCheck size={56} aria-hidden="true" /></span>
            <p ref={message} className={styles.message} role="status" tabIndex={-1}>
              {mode === "edit"
                ? text("Your event was updated successfully.")
                : approved
                  ? text("Your event was created and is now live.")
                  : text("Your event was created successfully. You'll be notified once it's approved. Track its status in \"My Events\"")}
            </p>
          </>
        )}
        <div className={styles.actions}>
          <Button variant="inverse" className={styles.action} onClick={() => { void navigate("/events"); }}>{text("Back to Explore")}</Button>
          <Button className={styles.action} onClick={() => { void navigate("/events?tab=my"); }}>{text("My Events")}</Button>
        </div>
      </div>
    </section>
  );
}
