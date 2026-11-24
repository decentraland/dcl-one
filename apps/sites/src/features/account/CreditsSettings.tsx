import { useEffect, useId, useRef, useState } from "react";

import { Button, Skeleton } from "@dcl/one-ui";
import { LeaveCreditsDialog } from "@dcl/one-ui/apps/sites";

import { optOutFailureMessage } from "@/data/lib/account/credits";
import type { AuthIdentity } from "@/data/lib/auth/types";

import { CREDITS_COPY } from "./copy";
import styles from "./CreditsSettings.module.css";
import { useCreditsStatus, useOptOutOfCredits } from "./hooks";

const COPY = CREDITS_COPY;

/**
 * `/account/credits`: the member's status in the Marketplace Credits program. Joining happens in-world,
 * so the web offers only leaving, behind a confirmation dialog the member can dismiss.
 */
export function CreditsSettings({ identity }: { identity: AuthIdentity }) {
  const titleId = useId();
  const status = useCreditsStatus(identity);
  const optOut = useOptOutOfCredits();
  const [confirming, setConfirming] = useState(false);
  const statusRef = useRef<HTMLParagraphElement>(null);
  const leaveRef = useRef<HTMLButtonElement>(null);

  // After leaving, focus moves to what the re-read shows once it renders: the new status or its failure.
  const [left, setLeft] = useState(0);
  useEffect(() => {
    if (left) statusRef.current?.focus();
  }, [left]);

  // A failed read, the re-read after leaving included, shows the failure, not the last status it held.
  const current = status.isError ? undefined : status.data?.status;
  const enrolled = current === "enrolled";

  const cancel = () => {
    optOut.reset();
    setConfirming(false);
  };

  const confirm = () => {
    if (optOut.isPending) return;
    optOut.mutate(
      { identity },
      {
        onSuccess: () => {
          setConfirming(false);
          setLeft((count) => count + 1);
        },
      },
    );
  };

  return (
    <section className={styles.root} aria-labelledby={titleId}>
      <div className={styles.card}>
        <h2 id={titleId} className={styles.title}>
          {COPY.title}
        </h2>
        {status.isPending ? (
          <div aria-busy="true">
            <Skeleton tone="neutral" className={styles.skeleton} />
          </div>
        ) : current ? (
          <p ref={statusRef} className={styles.status} tabIndex={-1} aria-live="polite">
            {COPY.statusLabel}{" "}
            <strong className={enrolled ? styles.enrolled : styles.statusValue}>{COPY.status[current]}</strong>
          </p>
        ) : (
          <div className={styles.loadError}>
            <p ref={statusRef} className={styles.error} role="alert" tabIndex={-1}>
              {COPY.loadError}
            </p>
            <Button variant="secondary" size="sm" onClick={() => void status.refetch()}>
              {COPY.retry}
            </Button>
          </div>
        )}
        <p className={styles.description}>
          {COPY.description}{" "}
          <a className={styles.link} href={COPY.learnMoreHref}>
            {COPY.learnMore}
          </a>
        </p>

        {enrolled ? (
          <div className={styles.actions}>
            <Button ref={leaveRef} size="md" onClick={() => setConfirming(true)}>
              {COPY.leave}
            </Button>
          </div>
        ) : null}
      </div>
      {enrolled && confirming ? (
        <LeaveCreditsDialog
          labels={COPY}
          pending={optOut.isPending}
          error={optOut.isError ? optOutFailureMessage(optOut.error) : undefined}
          onConfirm={confirm}
          onClose={cancel}
          returnFocusRef={leaveRef}
        />
      ) : null}
    </section>
  );
}
