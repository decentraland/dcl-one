import type { ChangeEvent, ReactNode } from "react";
import { createContext, useContext, useId, useState } from "react";
import styles from "./Radio.module.css";

type RadioChange = (value: string, event: ChangeEvent<HTMLInputElement>) => void;

type GroupState = {
  name: string;
  value: string | undefined;
  disabled: boolean;
  select: RadioChange;
};

const GroupContext = createContext<GroupState | null>(null);

export type RadioGroupProps = {
  value?: string;
  defaultValue?: string;
  onChange?: RadioChange;
  name?: string;
  // A visible name: the group is then a `fieldset` and the label its `legend`.
  label?: ReactNode;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  row?: boolean;
  disabled?: boolean;
  className?: string;
  children?: ReactNode;
};

export function RadioGroup({
  value,
  defaultValue,
  onChange,
  name,
  label,
  row = false,
  disabled = false,
  className,
  children,
  ...naming
}: RadioGroupProps) {
  const autoName = useId();
  const [internal, setInternal] = useState(defaultValue);
  const isControlled = value !== undefined;

  const select: RadioChange = (next, event) => {
    if (!isControlled) setInternal(next);
    onChange?.(next, event);
  };

  const state: GroupState = {
    name: name ?? autoName,
    value: isControlled ? value : internal,
    disabled,
    select,
  };
  const classes = className ? styles.group + " " + className : styles.group;

  return (
    <GroupContext.Provider value={state}>
      {label ? (
        <fieldset role="radiogroup" className={classes} data-row={row ? "" : undefined} disabled={disabled} {...naming}>
          <legend className={styles.legend}>{label}</legend>
          {children}
        </fieldset>
      ) : (
        <div
          role="radiogroup"
          className={classes}
          data-row={row ? "" : undefined}
          aria-disabled={disabled || undefined}
          {...naming}
        >
          {children}
        </div>
      )}
    </GroupContext.Provider>
  );
}

export type RadioProps = {
  value: string;
  // Outside a RadioGroup the caller owns the state.
  checked?: boolean;
  onChange?: RadioChange;
  name?: string;
  disabled?: boolean;
  size?: "small" | "medium";
  id?: string;
  "aria-label"?: string;
  className?: string;
  children?: ReactNode;
};

export default function Radio({
  value,
  checked,
  onChange,
  name,
  disabled = false,
  size = "medium",
  id,
  className,
  children,
  "aria-label": ariaLabel,
}: RadioProps) {
  const group = useContext(GroupContext);
  const isDisabled = disabled || Boolean(group?.disabled);

  return (
    <label
      className={className ? styles.radio + " " + className : styles.radio}
      data-size={size}
      data-disabled={isDisabled ? "" : undefined}
    >
      <input
        type="radio"
        className={styles.input}
        id={id}
        name={group ? group.name : name}
        value={value}
        checked={group ? group.value === value : checked}
        disabled={isDisabled}
        aria-label={ariaLabel}
        onChange={(event) => {
          group?.select(value, event);
          onChange?.(value, event);
        }}
      />
      <span className={styles.dot} aria-hidden="true" />
      {children != null ? <span>{children}</span> : null}
    </label>
  );
}
