import type { InputHTMLAttributes, ReactNode, Ref, TextareaHTMLAttributes } from 'react';
import { cx } from '/@/lib/cx';
import TextField from '@ui/atoms/TextField';

import styles from './TextInput.module.css';

type ControlElement = HTMLInputElement | HTMLTextAreaElement;

export type TextInputProps = Omit<InputHTMLAttributes<ControlElement>, 'size'> & {
  /** Attributes for the control itself. They win over the same attribute on the component. */
  inputProps?: Omit<InputHTMLAttributes<ControlElement>, 'size'> & {
    [name: `data-${string}`]: string | undefined;
  };
  /** Accepted so a caller that names the native element still compiles. The control is always native. */
  inputComponent?: unknown;
  inputRef?: Ref<ControlElement>;
  startAdornment?: ReactNode;
  endAdornment?: ReactNode;
  multiline?: boolean;
  minRows?: number;
  maxRows?: number;
  error?: boolean;
  fullWidth?: boolean;
};

type Look = 'form' | 'inline';

function Field({
  look,
  inputProps,
  inputComponent: _inputComponent,
  inputRef,
  startAdornment,
  endAdornment,
  multiline = false,
  minRows,
  maxRows,
  error,
  fullWidth,
  className,
  ...props
}: TextInputProps & { look: Look }) {
  const shared = {
    startAdornment,
    endAdornment,
    error,
    fullWidth,
    // `TextInput` is the name that the styles of a caller use for the field.
    className: cx(
      'TextInput',
      styles.control,
      styles[look],
      { [styles.multiline]: multiline },
      className,
    ),
  };
  const control = { ...props, ...inputProps };

  if (multiline) {
    return (
      <TextField
        {...(control as Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, 'rows' | 'className'>)}
        {...shared}
        multiline
        minRows={minRows}
        maxRows={maxRows}
        inputRef={inputRef as Ref<HTMLTextAreaElement>}
      />
    );
  }

  return (
    <TextField
      {...(control as Omit<InputHTMLAttributes<HTMLInputElement>, 'size' | 'className'>)}
      {...shared}
      inputRef={inputRef as Ref<HTMLInputElement>}
    />
  );
}

/** The 46px text box of a form. */
export function FormTextInput(props: TextInputProps) {
  return (
    <Field
      look="form"
      {...props}
    />
  );
}

/** The 40px text box of a row or a toolbar. */
export function TextInput(props: TextInputProps) {
  return (
    <Field
      look="inline"
      {...props}
    />
  );
}
