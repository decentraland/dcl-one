import type { ButtonHTMLAttributes } from "react";
import { forwardRef } from "react";
import styles from "./IconButton.module.css";

// An icon has no text, so the type asks for a name.
type Named =
  | { "aria-label": string; "aria-labelledby"?: string }
  | { "aria-label"?: string; "aria-labelledby": string };

export type IconButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-label" | "aria-labelledby"> &
  Named & {
    size?: "small" | "medium" | "large";
    edge?: "start" | "end";
    tone?: "default" | "inherit" | "primary";
  };

const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { size = "medium", edge, tone = "default", type = "button", className, children, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      type={type}
      className={className ? styles.root + " " + className : styles.root}
      data-size={size}
      data-edge={edge}
      data-tone={tone}
      {...rest}
    >
      {children}
    </button>
  );
});

export default IconButton;
