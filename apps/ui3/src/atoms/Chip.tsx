import type { MouseEventHandler, ReactNode } from "react";
import styles from "./Chip.module.css";

export type ChipProps = {
  label: ReactNode;
  variant?: "filled" | "outlined";
  size?: "small" | "medium";
  icon?: ReactNode;
  // With a handler the chip is a button; without one it is static text.
  onClick?: MouseEventHandler<HTMLButtonElement>;
  disabled?: boolean;
  className?: string;
  title?: string;
  "aria-label"?: string;
  "aria-pressed"?: boolean;
};

export default function Chip({
  label,
  variant = "filled",
  size = "medium",
  icon,
  onClick,
  disabled = false,
  className,
  title,
  "aria-label": ariaLabel,
  "aria-pressed": ariaPressed,
}: ChipProps) {
  const content = (
    <>
      {icon ? (
        <span className={styles.icon} aria-hidden="true">
          {icon}
        </span>
      ) : null}
      <span className={styles.label}>{label}</span>
    </>
  );
  const classes = styles.root + (onClick ? " " + styles.action : "") + (className ? " " + className : "");

  if (onClick) {
    return (
      <button
        type="button"
        className={classes}
        data-variant={variant}
        data-size={size}
        title={title}
        aria-label={ariaLabel}
        aria-pressed={ariaPressed}
        disabled={disabled}
        onClick={onClick}
      >
        {content}
      </button>
    );
  }
  return (
    <span className={classes} data-variant={variant} data-size={size} title={title} aria-label={ariaLabel}>
      {content}
    </span>
  );
}
