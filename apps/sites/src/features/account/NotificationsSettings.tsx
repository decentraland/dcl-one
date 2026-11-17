import { useId, useState, type FormEvent } from "react";

import { Button, InputWebCreatorTools, Skeleton, Switch } from "@dcl/one-ui";
import { GeneralDownArrow } from "@dcl/one-ui/foundations/icons";

import {
  isAllEmailEnabled,
  isEmailAddress,
  isTypeEmailEnabled,
  setAllEmail,
  setTypeEmail,
  SUBSCRIPTION_GROUP_COLUMNS,
  SUBSCRIPTION_GROUPS,
  type SubscriptionDetails,
  type SubscriptionGroupKey,
} from "@/data/lib/account/notifications";
import type { AuthIdentity } from "@/data/lib/auth/types";

import { GROUP_COPY, NOTIFICATIONS_COPY, TYPE_LABELS } from "./copy";
import { useSetEmail, useSubscription, useUpdateSubscription } from "./hooks";
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
        {email && details ? (
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
          disabled={loading}
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

type GroupCardProps = {
  group: SubscriptionGroupKey;
  details: SubscriptionDetails;
  disabled: boolean;
  onToggleType: (type: string, enabled: boolean) => void;
};

function GroupCard({ group, details, disabled, onToggleType }: GroupCardProps) {
  const copy = GROUP_COPY[group];
  return (
    <details className={styles.group}>
      <summary className={styles.groupSummary}>
        <span className={styles.groupText}>
          <span className={styles.groupTitle}>{copy.label}</span>
          <span className={styles.groupDescription}>{copy.description}</span>
        </span>
        <GeneralDownArrow className={styles.chevron} size={24} aria-hidden="true" />
      </summary>
      <ul className={styles.types}>
        {SUBSCRIPTION_GROUPS[group].map((type) => (
          <li key={type}>
            <Switch
              className={styles.typeSwitch}
              label={TYPE_LABELS[type] ?? type}
              checked={isTypeEmailEnabled(details, type)}
              disabled={disabled}
              onCheckedChange={(enabled) => onToggleType(type, enabled)}
            />
          </li>
        ))}
      </ul>
    </details>
  );
}

/** `/account/notifications`: the email address and which notifications it gets. */
export function NotificationsSettings({ identity }: { identity: AuthIdentity }) {
  const titleId = useId();
  const subscription = useSubscription(identity);
  const update = useUpdateSubscription();
  const data = subscription.data;
  const details = data?.details;
  const email = data?.email || "";
  const unconfirmedEmail = data?.unconfirmedEmail || "";
  const confirmed = !!email && !unconfirmedEmail;

  // Each write sends the details as shown, pending writes included, so quick toggles build on each other.
  const save = (next: SubscriptionDetails) => update.mutate({ identity, details: next });

  return (
    <section className={styles.root} aria-labelledby={titleId}>
      <header className={styles.header}>
        <h2 id={titleId} className={styles.title}>
          {COPY.title}
        </h2>
        <p className={styles.description}>{COPY.description}</p>
      </header>

      <EmailCard
        identity={identity}
        email={email}
        unconfirmedEmail={unconfirmedEmail}
        details={details}
        loading={subscription.isPending}
        onToggleAll={(enabled) => details && save(setAllEmail(details, enabled))}
      />

      {subscription.isError && !details ? (
        <p className={styles.error} role="alert">
          {COPY.loadError}
        </p>
      ) : null}
      {update.isError ? (
        <p className={styles.error} role="alert">
          {COPY.saveError}
        </p>
      ) : null}

      {details ? (
        <>
          {!confirmed ? <p className={styles.hint}>{COPY.confirmFirst}</p> : null}
          <div className={styles.columns}>
            {SUBSCRIPTION_GROUP_COLUMNS.map((column, index) => (
              <div className={styles.column} key={index}>
                {column.map((group) => (
                  <GroupCard
                    key={group}
                    group={group}
                    details={details}
                    disabled={!confirmed}
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
