import { useEffect, useRef, useState } from 'react';
import type { ChangeEvent, InputHTMLAttributes, ReactNode } from 'react';
import { cx } from '/@/lib/cx';
import { CheckIcon } from '@ui/atoms/icons-material';

import styles from './SelectCheckbox.module.css';

type InputProps = InputHTMLAttributes<HTMLInputElement> & { [attribute: `data-${string}`]: string };

/** The attributes of a native checkbox go to the input; `className` goes to the root, `inputProps` win. */
type Props = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  'type' | 'checked' | 'defaultChecked' | 'onChange' | 'children'
> & {
  checked?: boolean;
  defaultChecked?: boolean;
  indeterminate?: boolean;
  inputProps?: InputProps;
  onChange?: (event: ChangeEvent<HTMLInputElement>, checked: boolean) => void;
  variant?: 'primary' | 'secondary';
};

const FILLED_TICK =
  'M19 3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.11 0 2-.9 2-2V5c0-1.1-.89-2-2-2zm-9 14l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z';

const OFF = <span className={styles.off} />;

function onIcon(variant: Props['variant'], disabled?: boolean): ReactNode {
  if (variant === 'secondary')
    return (
      <span className={styles.on}>
        <CheckIcon />
      </span>
    );
  if (disabled)
    return (
      <span className={styles.fixedOn}>
        <CheckIcon />
      </span>
    );
  return (
    <svg
      className={styles.tick}
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
    >
      <path d={FILLED_TICK} />
    </svg>
  );
}

function mixedIcon(variant: Props['variant']): ReactNode {
  if (variant === 'secondary')
    return (
      <span className={styles.on}>
        <span className={styles.dash} />
      </span>
    );
  return (
    <span className={styles.mixed}>
      <span
        className={styles.mixedDash}
        data-dash
      />
    </span>
  );
}

/** The detail tabs' row-selection checkbox: a 20px outlined box off; on, the Primary tick (`primary`, grey when disabled; mixed, a filled Primary box with a dark dash) or a Primary-outlined box with a Primary tick and dash (`secondary`). */
export function SelectCheckbox({
  variant = 'primary',
  checked,
  defaultChecked,
  indeterminate = false,
  disabled,
  className,
  inputProps,
  onChange,
  ...inputAttributes
}: Props) {
  const input = useRef<HTMLInputElement>(null);
  const [own, setOwn] = useState(!!defaultChecked);
  const on = checked ?? own;

  useEffect(() => {
    if (input.current) input.current.indeterminate = indeterminate;
  }, [indeterminate]);

  return (
    <span
      className={cx(styles.root, className)}
      data-checked={on || undefined}
      data-disabled={disabled || undefined}
    >
      <input
        {...inputAttributes}
        {...inputProps}
        ref={input}
        className={cx(styles.input, inputProps?.className)}
        type="checkbox"
        data-indeterminate={indeterminate}
        checked={on}
        disabled={disabled}
        onChange={event => {
          if (checked === undefined) setOwn(event.target.checked);
          onChange?.(event, event.target.checked);
        }}
      />
      {indeterminate ? mixedIcon(variant) : on ? onIcon(variant, disabled) : OFF}
    </span>
  );
}
