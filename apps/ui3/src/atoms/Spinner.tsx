import type { ComponentPropsWithoutRef, CSSProperties } from "react";
import "./spinner.css";
import styles from "./Spinner.module.css";

type SpinnerProps = {
  size?: number;
  /** A CSS colour, or a keyword: "inherit" takes the text colour, "primary" the brand colour. */
  color?: string;
  /** The accessible name. Pass role="progressbar" where the host expects that role. */
  label?: string;
  /** "wide" draws a wider ring with butt ends and an arc that grows and shrinks. */
  ring?: "wide";
} & ComponentPropsWithoutRef<"span">;

const COLOR_KEYWORDS: Record<string, string> = { inherit: "currentColor", primary: "var(--brand)" };

export default function Spinner({ size = 28, color, label = "Loading", ring, className, style, ...rest }: SpinnerProps) {
  const vars: CSSProperties & { "--sz": string; "--spinner-arc"?: string } = { ...style, "--sz": size + "px" };
  if (color) vars["--spinner-arc"] = COLOR_KEYWORDS[color] ?? color;
  const classes = "spinner" + (ring === "wide" ? " " + styles.wide : "") + (className ? " " + className : "");
  return (
    <span className={classes} style={vars} role="status" aria-label={label} {...rest}>
      <svg viewBox="0 0 50 50" width={size} height={size}>
        <circle className="spinner__arc" cx="25" cy="25" r="20" fill="none" strokeWidth="4"
          strokeLinecap="round" strokeDasharray="90 160" />
      </svg>
    </span>
  );
}
