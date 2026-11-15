import type { ChangeEvent, ReactNode } from "react";
import { useEffect, useRef, useState } from "react";
import "./checkbox.css";

export type CheckboxProps = {
  checked?: boolean;
  defaultChecked?: boolean;
  onChange?: (checked: boolean, event: ChangeEvent<HTMLInputElement>) => void;
  children?: ReactNode;
  disabled?: boolean;
  indeterminate?: boolean;
  size?: "sm";
  // Replace the drawn box. `checkedIcon` falls back to `icon`.
  icon?: ReactNode;
  checkedIcon?: ReactNode;
  id?: string;
  name?: string;
  value?: string;
  ariaLabel?: string;
  className?: string;
};

export default function Checkbox({
  checked,
  defaultChecked = false,
  onChange,
  children,
  disabled = false,
  indeterminate = false,
  size,
  icon,
  checkedIcon,
  id,
  name,
  value,
  ariaLabel,
  className,
}: CheckboxProps) {
  const [internal, setInternal] = useState(defaultChecked);
  const input = useRef<HTMLInputElement>(null);
  const isControlled = checked !== undefined;
  const on = isControlled ? checked : internal;

  useEffect(() => {
    if (input.current) input.current.indeterminate = indeterminate;
  }, [indeterminate]);

  function toggle(event: ChangeEvent<HTMLInputElement>) {
    if (!isControlled) setInternal(!on);
    onChange?.(!on, event);
  }

  const custom = icon !== undefined || checkedIcon !== undefined;

  return (
    <label
      className={"checkbox" + (disabled ? " is-disabled" : "") + (className ? " " + className : "")}
      data-size={size}
    >
      <input
        ref={input}
        type="checkbox" className="checkbox__input"
        checked={on} onChange={toggle}
        disabled={disabled} id={id} name={name} value={value} aria-label={ariaLabel}
      />
      {custom ? (
        <span className="checkbox__icon" aria-hidden="true">{on ? (checkedIcon ?? icon) : icon}</span>
      ) : (
        <span className={"checkbox__box" + (on ? " is-checked" : "") + (indeterminate ? " is-indeterminate" : "")}>
          {indeterminate ? (
            <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">
              <path d="M3.5 8h9" fill="none" stroke="#fff" strokeWidth="2.2" strokeLinecap="round" />
            </svg>
          ) : on && (
            <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">
              <path d="M3 8.5l3 3 7-7" fill="none" stroke="#fff" strokeWidth="2.2"
                strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          )}
        </span>
      )}
      <span className="checkbox__label">{children}</span>
    </label>
  );
}
