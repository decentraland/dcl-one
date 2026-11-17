import { useId, useState, type ReactNode } from 'react';
import {
  VisibilityOutlinedIcon as VisibilityIcon,
  VisibilityOffOutlinedIcon as VisibilityOffIcon,
  WarningAmberIcon as WarningIcon,
} from '/@/components/Icons/material';
import IconButton from '@ui/atoms/IconButton';

import { t } from '/@/modules/store/translation/utils';

import { DialogButton } from '../../../DialogButton';
import { InfoIcon } from '../../../Icons';
import { TitleBarModal } from '../../../Modals/TitleBarModal';
import { FormTextInput } from '../../../TextInput';

import styles from './PasswordModal.module.css';

const MIN_LENGTH = 8;
const MIN_NUMBERS = 2;

type Props = {
  open: boolean;
  mode: 'create' | 'change';
  onCancel: () => void;
  onConfirm: (password: string) => void;
};

type FieldProps = {
  label: string;
  value: string;
  error: ReactNode;
  autoFocus?: boolean;
  onChange: (value: string) => void;
};

function PasswordField({ label, value, error, autoFocus, onChange }: FieldProps) {
  const inputId = useId();
  const errorId = useId();
  const [shown, setShown] = useState(false);
  return (
    <div className={styles.field}>
      <label
        className={styles.fieldLabel}
        htmlFor={inputId}
      >
        {label}
      </label>
      <FormTextInput
        id={inputId}
        type={shown ? 'text' : 'password'}
        value={value}
        autoFocus={autoFocus}
        error={!!error}
        onChange={event => onChange(event.target.value)}
        inputProps={{ 'aria-describedby': error ? errorId : undefined, 'aria-invalid': !!error }}
        endAdornment={
          <IconButton
            className={styles.eyeButton}
            aria-label={t(
              shown ? 'world_detail.settings.password.hide' : 'world_detail.settings.password.show',
            )}
            onClick={() => setShown(current => !current)}
          >
            {shown ? <VisibilityIcon /> : <VisibilityOffIcon />}
          </IconButton>
        }
      />
      {error && (
        <div
          className={styles.errorBlock}
          id={errorId}
          role="alert"
        >
          {error}
        </div>
      )}
    </div>
  );
}

/** Creates or changes a World's access password: 8+ characters with 2+ numbers, typed twice. */
export function PasswordModal({ open, mode, onCancel, onConfirm }: Props) {
  const [password, setPassword] = useState('');
  const [repeat, setRepeat] = useState('');
  const longEnough = password.length >= MIN_LENGTH;
  const enoughNumbers = (password.match(/\d/g) ?? []).length >= MIN_NUMBERS;
  const mismatch = !!password && !!repeat && password !== repeat;
  const valid = longEnough && enoughNumbers && password === repeat;

  const rulesError = password && (!longEnough || !enoughNumbers) && (
    <>
      <span>{t('world_detail.settings.password.requirements')}</span>
      <ul>
        {!longEnough && <li>{t('world_detail.settings.password.min_length')}</li>}
        {!enoughNumbers && <li>{t('world_detail.settings.password.min_numbers')}</li>}
      </ul>
    </>
  );

  return (
    <TitleBarModal
      open={open}
      title={t(
        mode === 'create'
          ? 'world_detail.settings.password.create_title'
          : 'world_detail.settings.password.change_title',
      )}
      onClose={onCancel}
    >
      <PasswordField
        label={t('world_detail.settings.password.type_password')}
        value={password}
        error={rulesError}
        autoFocus
        onChange={setPassword}
      />
      <PasswordField
        label={t('world_detail.settings.password.repeat_password')}
        value={repeat}
        error={
          mismatch && (
            <span className={styles.mismatch}>
              <WarningIcon />
              {t('world_detail.settings.password.mismatch')}
            </span>
          )
        }
        onChange={setRepeat}
      />
      <div className={styles.footer}>
        <p className={styles.info}>
          <InfoIcon />
          {t('world_detail.settings.password.info')}
        </p>
        <div className={styles.actions}>
          <DialogButton
            type="button"
            tone="secondary"
            onClick={onCancel}
          >
            {t('modal.cancel')}
          </DialogButton>
          <DialogButton
            type="button"
            tone="primary"
            disabled={!valid}
            onClick={() => onConfirm(password)}
          >
            {t('modal.confirm')}
          </DialogButton>
        </div>
      </div>
    </TitleBarModal>
  );
}
