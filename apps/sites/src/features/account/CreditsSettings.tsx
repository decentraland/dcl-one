import { useEffect, useId, useRef, useState } from "react";

import { Button, Skeleton } from "@dcl/one-ui";

import { optOutFailureMessage } from "@/data/lib/account/credits";
import type { AuthIdentity } from "@/data/lib/auth/types";

import { CREDITS_COPY } from "./copy";
import styles from "./CreditsSettings.module.css";
import { useCreditsStatus, useOptOutOfCredits } from "./hooks";

const COPY = CREDITS_COPY;

/**
 * `/account/credits`: the member's status in the Marketplace Credits program. Joining happens in-world,
 * so the web offers only leaving, behind a confirmation the member can cancel.
 */
export function CreditsSettings({ identity }: { identity: AuthIdentity }) {
  const titleId = useId();
  const confirmTitleId = useId();
  const status = useCreditsStatus(identity);
  const optOut = useOptOutOfCredits();
  const [confirming, setConfirming] = useState(false);
  const confirmRef = useRef<HTMLDivElement>(null);
  const leaveRef = useRef<HTMLButtonElement>(null);
  const returnFocus = useRef(false);

  useEffect(() => {
    if (confirming) confirmRef.current?.focus();
    else if (returnFocus.current) leaveRef.current?.focus();
    returnFocus.current = false;
  }, [confirming]);

  const current = status.data?.status;
  const enrolled = current === "enrolled";

  const cancel = () => {
    returnFocus.current = true;
    optOut.reset();
    setConfirming(false);
  };

  const confirm = () => {
    if (optOut.isPending) return;
    optOut.mutate({ identity }, { onSuccess: () => setConfirming(false) });
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
          <p className={styles.status}>
            {COPY.statusLabel}{" "}
            <strong className={enrolled ? styles.enrolled : styles.statusValue}>{COPY.status[current]}</strong>
          </p>
        ) : (
          <p className={styles.error} role="alert">
            {COPY.loadError}
          </p>
        )}
        <p className={styles.description}>
          {COPY.description}{" "}
          <a className={styles.link} href={COPY.learnMoreHref}>
            {COPY.learnMore}
          </a>
        </p>

        {enrolled && !confirming ? (
          <div className={styles.actions}>
            <Button ref={leaveRef} size="md" onClick={() => setConfirming(true)}>
              {COPY.leave}
            </Button>
          </div>
        ) : null}

        {enrolled && confirming ? (
          <div
            ref={confirmRef}
            className={styles.confirm}
            role="group"
            aria-labelledby={confirmTitleId}
            tabIndex={-1}
          >
            <h3 id={confirmTitleId} className={styles.confirmTitle}>
              {COPY.confirmTitle}
            </h3>
            <p className={styles.description}>{COPY.confirmDescription}</p>
            {optOut.isError ? (
              <p className={styles.error} role="alert">
                {optOutFailureMessage(optOut.error)}
              </p>
            ) : null}
            <div className={styles.actions}>
              <Button variant="secondary" size="md" onClick={cancel} disabled={optOut.isPending}>
                {COPY.cancel}
              </Button>
              <Button size="md" onClick={confirm} loading={optOut.isPending}>
                {COPY.confirm}
              </Button>
            </div>
          </div>
        ) : null}
      </div>
    </section>
  );
}
