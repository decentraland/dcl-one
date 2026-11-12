import { useEffect, useId, useRef, useState } from 'react';
import { Button, TextField } from 'decentraland-ui2';

import type { EnrollmentRequest } from '/shared/types/creator-fund';

import { t } from '/@/modules/store/translation/utils';
import { EMAIL_MAX_LENGTH, isEmail } from '/@/modules/utils';

import styles from './ContactInfoForm.module.css';
import modal from './EarnModal.module.css';

const NAME_MAX_LENGTH = 100;

export type ContactValues = Pick<EnrollmentRequest, 'first_name' | 'last_name' | 'email'>;
type ContactField = keyof ContactValues;

const EMPTY: ContactValues = { first_name: '', last_name: '', email: '' };

type FieldProps = {
  name: ContactField;
  label: string;
  placeholder: string;
  value: string;
  error?: string;
  maxLength: number;
  onChange: (value: string) => void;
  onBlur?: () => void;
  autoFocus?: boolean;
  type?: 'email';
  autoComplete: 'given-name' | 'family-name' | 'email';
};

function Field({ name, label, error, maxLength, onChange, autoFocus, ...input }: FieldProps) {
  const id = useId();
  const errorId = `${id}-error`;
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (autoFocus) inputRef.current?.focus();
  }, []);

  return (
    <div className={styles.fieldGroup}>
      <label
        className={styles.label}
        htmlFor={id}
      >
        {label}
        <span
          className={styles.required}
          aria-hidden
        >
          {' *'}
        </span>
      </label>
      <TextField
        className={styles.input}
        {...input}
        inputRef={inputRef}
        id={id}
        name={name}
        required
        error={!!error}
        onChange={event => onChange(event.target.value)}
        inputProps={{
          maxLength,
          spellCheck: input.type === 'email' ? false : undefined,
          'aria-describedby': error ? errorId : undefined,
        }}
      />
      <span
        className={styles.fieldError}
        id={errorId}
        aria-live="polite"
      >
        {error}
      </span>
    </div>
  );
}

export type ContactInfoFormProps = {
  titleId?: string;
  onSubmit: (values: ContactValues) => void;
  onCancel: () => void;
  errors?: string[];
  status?: 'idle' | 'submitting';
  /** What the form starts with, e.g. the details sent before the flow went back to the terms. */
  initialValues?: ContactValues;
};

/** The Creator Fund contact form: three required fields, submitted trimmed. */
export function ContactInfoForm({
  titleId,
  onSubmit,
  onCancel,
  errors,
  status = 'idle',
  initialValues = EMPTY,
}: ContactInfoFormProps) {
  const [values, setValues] = useState<ContactValues>(initialValues);
  const [emailBlurred, setEmailBlurred] = useState(false);

  const set = (field: ContactField) => (value: string) =>
    setValues(current => ({ ...current, [field]: value }));

  const emailError =
    emailBlurred && values.email && !isEmail(values.email)
      ? t('earn.contact.invalid_email')
      : undefined;

  const trimmed: ContactValues = {
    first_name: values.first_name.trim(),
    last_name: values.last_name.trim(),
    email: values.email.trim(),
  };
  const valid = !!trimmed.first_name && !!trimmed.last_name && isEmail(trimmed.email);

  const handleSubmit = (event: React.FormEvent) => {
    event.preventDefault();
    if (!valid || status === 'submitting') return;
    onSubmit(trimmed);
  };

  return (
    <form
      className={styles.form}
      noValidate
      onSubmit={handleSubmit}
    >
      <h2
        className={modal.modalTitle}
        id={titleId}
      >
        {t('earn.contact.title')}
      </h2>
      <p className={styles.intro}>{t('earn.contact.body')}</p>
      <div className={styles.fields}>
        <div className={styles.nameRow}>
          <Field
            name="first_name"
            label={t('earn.contact.first_name')}
            placeholder={t('earn.contact.first_name_placeholder')}
            autoComplete="given-name"
            value={values.first_name}
            maxLength={NAME_MAX_LENGTH}
            onChange={set('first_name')}
            autoFocus
          />
          <Field
            name="last_name"
            label={t('earn.contact.last_name')}
            placeholder={t('earn.contact.last_name_placeholder')}
            autoComplete="family-name"
            value={values.last_name}
            maxLength={NAME_MAX_LENGTH}
            onChange={set('last_name')}
          />
        </div>
        <Field
          name="email"
          type="email"
          label={t('earn.contact.email')}
          placeholder={t('earn.contact.email_placeholder')}
          autoComplete="email"
          value={values.email}
          error={emailError}
          maxLength={EMAIL_MAX_LENGTH}
          onChange={set('email')}
          onBlur={() => setEmailBlurred(true)}
        />
      </div>
      {errors?.length ? (
        <div role="alert">
          {errors.map(line => (
            <p
              className={modal.errorLine}
              key={line}
            >
              {line}
            </p>
          ))}
        </div>
      ) : null}
      <div className={modal.actions}>
        <Button
          className={modal.modalCta}
          type="button"
          variant="contained"
          color="secondary"
          size="large"
          onClick={onCancel}
        >
          {t('earn.contact.cancel')}
        </Button>
        <Button
          className={modal.modalCta}
          type="submit"
          variant="contained"
          color="primary"
          size="large"
          disabled={!valid || status === 'submitting'}
          aria-busy={status === 'submitting' || undefined}
        >
          {t('earn.contact.submit')}
        </Button>
      </div>
    </form>
  );
}
