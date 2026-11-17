import { useId, useState, type FormEvent } from "react";

import { Button, InputWebCreatorTools, Skeleton, Switch } from "@dcl/one-ui";

import {
  isAllEmailEnabled,
  isEmailAddress,
  setAllEmail,
  setTypeEmail,
  SUBSCRIPTION_GROUP_COLUMNS,
  type SubscriptionDetails,
} from "@/data/lib/account/notifications";
import type { AuthIdentity } from "@/data/lib/auth/types";

import { NOTIFICATIONS_COPY } from "./copy";
import { useSetEmail, useSubscription, useUpdateSubscription } from "./hooks";
import { NotificationGroupAccordion } from "./NotificationGroupAccordion";
import styles from "./NotificationsSettings.module.css";

const COPY = NOTIFICATIONS_COPY;

type EmailCardProps = {
  identity: AuthIdentity;
  email: string;
  unconfirmedEmail: string;
  details: SubscriptionDetails | undefined;
  loading: boolean;
  onToggleAll: (enabled: boolean) => void;
};

function emailDescription(email: string, unconfirmed: string): string {
  if (!email && !unconfirmed) return COPY.email.withoutEmail;
  if (unconfirmed) return COPY.email.pendingApproval;
  return COPY.email.confirmedDescription;
}

/**
 * The address and the master switch. The field is a draft seeded from the server's address: an edit
 * survives a re-read, a failed save keeps it, and a saved one gives way to what the server now holds.
 */
function EmailCard({ identity, email, unconfirmedEmail, details, loading, onToggleAll }: EmailCardProps) {
  const titleId = useId();
  const setEmail = useSetEmail();
  const upstream = unconfirmedEmail || email;
  const [draft, setDraft] = useState<string | null>(null);
  const [invalid, setInvalid] = useState(false);
  const value = draft ?? upstream;
  const trimmed = value.trim();

  const confirmed = !!email && !unconfirmedEmail;
  const label = !upstream
    ? COPY.email.submit
    : unconfirmedEmail && unconfirmedEmail === trimmed
      ? COPY.email.resend
      : COPY.email.save;
  const unchanged = trimmed === email && !unconfirmedEmail;
  const saveDisabled = loading || trimmed === "" || unchanged;

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (saveDisabled || setEmail.isPending) return;
    if (!isEmailAddress(trimmed)) {
      setInvalid(true);
      return;
    }
    setEmail.mutate({ identity, email: trimmed }, { onSuccess: () => setDraft(null) });
  };

  const error = invalid ? COPY.email.invalid : setEmail.isError ? COPY.email.error : undefined;

  return (
    <section className={styles.card} aria-labelledby={titleId}>
      <div className={styles.cardHeader}>
        <h3 id={titleId} className={styles.cardTitle}>
          {COPY.email.title}
          {upstream ? (
            <span className={confirmed ? styles.badgeConfirmed : styles.badgePending}>
              {confirmed ? COPY.email.confirmed : COPY.email.pending}
            </span>
          ) : null}
        </h3>
        {confirmed && details ? (
          <Switch
            label={COPY.email.master}
            labelHidden
            checked={isAllEmailEnabled(details)}
            onCheckedChange={onToggleAll}
          />
        ) : null}
      </div>
      <p className={styles.cardDescription}>{emailDescription(email, unconfirmedEmail)}</p>
      <form className={styles.emailForm} onSubmit={submit} noValidate>
        <InputWebCreatorTools
          label={COPY.email.label}
          labelHidden
          type="email"
          name="email"
          autoComplete="email"
          placeholder={COPY.email.placeholder}
          value={value}
          disabled={loading || setEmail.isPending}
          error={error}
          onChange={(event) => {
            setDraft(event.currentTarget.value);
            setInvalid(false);
            if (setEmail.isError) setEmail.reset();
          }}
          action={
            <Button type="submit" size="md" disabled={saveDisabled} loading={setEmail.isPending}>
              {label}
            </Button>
          }
        />
      </form>
    </section>
  );
}

/** `/account/notifications`: the email card, then each notification category to open and switch per type. */
export function NotificationsSettings({ identity }: { identity: AuthIdentity }) {
  const titleId = useId();
  const subscription = useSubscription(identity);
  const update = useUpdateSubscription();
  const data = subscription.data;
  const details = data?.details;
  const email = data?.email || "";
  const unconfirmedEmail = data?.unconfirmedEmail || "";
  const confirmed = !!email && !unconfirmedEmail;
  const muted = !!details && !isAllEmailEnabled(details);
  const typesLocked = !confirmed || muted;

  // Each write sends the details as shown, pending writes included, so quick toggles build on each other.
  const save = (next: SubscriptionDetails) => update.mutate({ identity, details: next });

  return (
    <section className={styles.root} aria-labelledby={titleId}>
      <h2 id={titleId} className={styles.srOnly}>
        {COPY.title}
      </h2>

      <EmailCard
        identity={identity}
        email={email}
        unconfirmedEmail={unconfirmedEmail}
        details={details}
        loading={subscription.isPending || !details}
        onToggleAll={(enabled) => details && save(setAllEmail(details, enabled))}
      />

      {subscription.isError && !details ? (
        <div className={styles.loadError}>
          <p className={styles.error} role="alert">
            {COPY.loadError}
          </p>
          <Button variant="secondary" size="sm" onClick={() => void subscription.refetch()}>
            {COPY.retry}
          </Button>
        </div>
      ) : null}
      {update.isError ? (
        <p className={styles.error} role="alert">
          {COPY.saveError}
        </p>
      ) : null}

      {details ? (
        <>
          {!confirmed ? (
            <p className={styles.hint}>{COPY.confirmFirst}</p>
          ) : muted ? (
            <p className={styles.hint}>{COPY.unmuteFirst}</p>
          ) : null}
          <div className={styles.columns}>
            {SUBSCRIPTION_GROUP_COLUMNS.map((column, index) => (
              <div className={styles.column} key={index}>
                {column.map((group) => (
                  <NotificationGroupAccordion
                    key={group}
                    group={group}
                    details={details}
                    disabled={typesLocked}
                    onToggleType={(type, enabled) => save(setTypeEmail(details, type, enabled))}
                  />
                ))}
              </div>
            ))}
          </div>
        </>
      ) : subscription.isPending ? (
        <div className={styles.loading} aria-busy="true">
          <span className={styles.srOnly}>{COPY.loading}</span>
          <Skeleton tone="neutral" className={styles.skeleton} />
          <Skeleton tone="neutral" className={styles.skeleton} />
          <Skeleton tone="neutral" className={styles.skeleton} />
        </div>
      ) : null}
    </section>
  );
}
