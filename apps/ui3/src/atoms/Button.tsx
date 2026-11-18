import type { ComponentPropsWithoutRef, ForwardedRef, ReactNode } from "react";
import { forwardRef } from "react";
import "./button.css";
import styles from "./Button.module.css";

// The primary, secondary and ghost looks are the classes of button.css. The text and outline looks, the
// neutral tone, the icon slots and the block width are classes of Button.module.css: a button that does not
// ask for them has the markup and the looks of button.css alone.
type ButtonLook = {
  variant?: "primary" | "secondary" | "ghost" | "text" | "outline";
  size?: "sm" | "md" | "lg";
  tone?: "danger" | "success" | "warning" | "neutral";
  /** Decorative icons before and after the label, hidden from assistive technology. */
  startIcon?: ReactNode;
  endIcon?: ReactNode;
  /** The button fills the width of its container. */
  fullWidth?: boolean;
};

export type ButtonProps =
  | (ButtonLook & { as?: "button" } & ComponentPropsWithoutRef<"button">)
  | (ButtonLook & { as: "a" } & ComponentPropsWithoutRef<"a">);

const LOOKS = {
  primary: "btn--primary",
  secondary: "btn--secondary",
  ghost: "btn--ghost",
  text: styles.text,
  outline: styles.outline,
};

function classes(look: ButtonLook, className: string, gated = false) {
  const { variant = "primary", size = "md", tone, fullWidth = false } = look;
  return [
    "btn",
    LOOKS[variant],
    "btn--" + size,
    tone === "neutral" ? styles.neutral : "",
    fullWidth ? styles.block : "",
    gated ? "is-disabled" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");
}

function content(children: ReactNode, startIcon: ReactNode, endIcon: ReactNode) {
  if (!startIcon && !endIcon) return children;
  return (
    <>
      {startIcon ? <span className={styles.icon} aria-hidden="true">{startIcon}</span> : null}
      {children}
      {endIcon ? <span className={styles.icon} aria-hidden="true">{endIcon}</span> : null}
    </>
  );
}

const Button = forwardRef<HTMLButtonElement | HTMLAnchorElement, ButtonProps>(function Button(props, ref) {
  if (props.as === "a") {
    const { as: _as, variant, size, tone, startIcon, endIcon, fullWidth, className = "", children, ...rest } = props;
    const gated = rest["aria-disabled"] === true || rest["aria-disabled"] === "true";
    return (
      <a
        ref={ref as ForwardedRef<HTMLAnchorElement>}
        className={classes({ variant, size, tone, fullWidth }, className, gated)}
        data-tone={tone}
        {...rest}
      >
        {content(children, startIcon, endIcon)}
      </a>
    );
  }

  const {
    as: _as,
    variant,
    size,
    tone,
    startIcon,
    endIcon,
    fullWidth,
    disabled = false,
    type = "button",
    className = "",
    children,
    ...rest
  } = props;
  return (
    <button
      ref={ref as ForwardedRef<HTMLButtonElement>}
      type={type}
      className={classes({ variant, size, tone, fullWidth }, className)}
      data-tone={tone}
      disabled={disabled}
      {...rest}
    >
      {content(children, startIcon, endIcon)}
    </button>
  );
});

export default Button;
