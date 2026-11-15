import type { CSSProperties, InputHTMLAttributes, ReactNode, Ref, TextareaHTMLAttributes } from "react";
import { useId } from "react";
import styles from "./TextField.module.css";

type Shared = {
  label?: ReactNode;
  helperText?: ReactNode;
  error?: boolean;
  fullWidth?: boolean;
  size?: "small" | "medium";
  tone?: "primary" | "neutral";
  startAdornment?: ReactNode;
  endAdornment?: ReactNode;
  className?: string;
};

type SingleLine = Shared & {
  multiline?: false;
  inputRef?: Ref<HTMLInputElement>;
} & Omit<InputHTMLAttributes<HTMLInputElement>, "size" | "className">;

type MultiLine = Shared & {
  multiline: true;
  minRows?: number;
  maxRows?: number;
  inputRef?: Ref<HTMLTextAreaElement>;
} & Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "rows" | "className">;

export type TextFieldProps = SingleLine | MultiLine;

function native<T extends Shared>(props: T): Omit<T, keyof Shared> {
  const {
    label: _label,
    helperText: _helperText,
    error: _error,
    fullWidth: _fullWidth,
    size: _size,
    tone: _tone,
    startAdornment: _startAdornment,
    endAdornment: _endAdornment,
    className: _className,
    ...rest
  } = props;
  return rest;
}

export default function TextField(props: TextFieldProps) {
  const autoId = useId();
  const {
    label,
    helperText,
    error = false,
    fullWidth = false,
    size = "medium",
    tone = "primary",
    startAdornment,
    endAdornment,
    className,
    disabled = false,
    id = autoId,
  } = props;
  const helperId = helperText ? id + "-help" : undefined;
  const describedBy = [props["aria-describedby"], helperId].filter(Boolean).join(" ") || undefined;
  const invalid = error ? true : props["aria-invalid"];

  let control: ReactNode;
  if (props.multiline) {
    const { multiline: _multiline, minRows = 1, maxRows, inputRef, ...rest } = native(props);
    // The rows grow with the content between the two limits where the browser has `field-sizing`.
    const rows: CSSProperties & { "--field-min-rows": number; "--field-max-rows"?: number } = {
      ...rest.style,
      "--field-min-rows": minRows,
    };
    if (maxRows !== undefined) rows["--field-max-rows"] = maxRows;
    control = (
      <textarea
        {...rest}
        ref={inputRef}
        id={id}
        rows={minRows}
        className={styles.input + " " + styles.area}
        style={rows}
        disabled={disabled}
        aria-invalid={invalid}
        aria-describedby={describedBy}
      />
    );
  } else {
    const { multiline: _multiline, inputRef, type = "text", ...rest } = native(props);
    control = (
      <input
        {...rest}
        ref={inputRef}
        id={id}
        type={type}
        className={styles.input}
        disabled={disabled}
        aria-invalid={invalid}
        aria-describedby={describedBy}
      />
    );
  }

  return (
    <div
      className={className ? styles.root + " " + className : styles.root}
      data-size={size}
      data-tone={tone}
      data-full-width={fullWidth ? "" : undefined}
      data-error={error ? "" : undefined}
      data-disabled={disabled ? "" : undefined}
    >
      {label ? (
        <label className={styles.label} htmlFor={id}>
          {label}
        </label>
      ) : null}
      <div className={styles.field}>
        {startAdornment ? <span className={styles.adornment}>{startAdornment}</span> : null}
        {control}
        {endAdornment ? <span className={styles.adornment}>{endAdornment}</span> : null}
      </div>
      {helperText ? (
        <p id={helperId} className={styles.helper}>
          {helperText}
        </p>
      ) : null}
    </div>
  );
}
