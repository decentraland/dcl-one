import type { ComponentPropsWithoutRef, ForwardedRef, ReactNode } from "react";
import { forwardRef } from "react";
import "./button.css";
import styles from "./HubButton.module.css";

// The Creator Hub variation of Button. It keeps the classes of button.css for the looks that file has and
// takes the rest from its own module: the text and outline looks, the neutral tone, the icon slots, the
// block width, the --ctl-fs-* label sizes and the upper-case labels of --ctl-caps.
type HubButtonLook = {
  variant?: "primary" | "secondary" | "ghost" | "text" | "outline";
  size?: "sm" | "md" | "lg";
  tone?: "danger" | "success" | "warning" | "neutral";
  startIcon?: ReactNode;
  endIcon?: ReactNode;
  fullWidth?: boolean;
};

export type HubButtonProps =
  | (HubButtonLook & { as?: "button" } & ComponentPropsWithoutRef<"button">)
  | (HubButtonLook & { as: "a" } & ComponentPropsWithoutRef<"a">);

const LOOKS = {
  primary: "btn--primary",
  secondary: "btn--secondary",
  ghost: "btn--ghost",
  text: styles.text,
  outline: styles.outline,
};

function classes(variant: keyof typeof LOOKS, size: string, className: string, extra = "") {
  return "btn " + LOOKS[variant] + " btn--" + size + " " + styles.root + extra + (className ? " " + className : "");
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

const HubButton = forwardRef<HTMLButtonElement | HTMLAnchorElement, HubButtonProps>(function HubButton(props, ref) {
  if (props.as === "a") {
    const {
      as: _as,
      variant = "primary",
      size = "md",
      tone,
      startIcon,
      endIcon,
      fullWidth = false,
      className = "",
      children,
      ...rest
    } = props;
    const gated = rest["aria-disabled"] === true || rest["aria-disabled"] === "true";
    const extra = (gated ? " is-disabled" : "") + (fullWidth ? " " + styles.block : "");
    return (
      <a
        ref={ref as ForwardedRef<HTMLAnchorElement>}
        className={classes(variant, size, className, extra)}
        data-tone={tone}
        {...rest}
      >
        {content(children, startIcon, endIcon)}
      </a>
    );
  }

  const {
    as: _as,
    variant = "primary",
    size = "md",
    tone,
    startIcon,
    endIcon,
    fullWidth = false,
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
      className={classes(variant, size, className, fullWidth ? " " + styles.block : "")}
      data-tone={tone}
      disabled={disabled}
      {...rest}
    >
      {content(children, startIcon, endIcon)}
    </button>
  );
});

export default HubButton;
