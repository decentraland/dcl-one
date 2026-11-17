import type { ComponentPropsWithoutRef } from "react";
import Spinner from "./Spinner";
import styles from "./HubSpinner.module.css";

// The Creator Hub variation of Spinner: a name of its own, colour keywords and the wide ring.
type HubSpinnerProps = {
  size?: number;
  // A CSS colour, or a keyword: "inherit" takes the text colour, "primary" the brand colour.
  color?: string;
  // The accessible name. Pass role="progressbar" where the host expects that role.
  label?: string;
  // "wide" draws a wider ring with butt ends and an arc that grows and shrinks. Without it the ring of Spinner stays.
  ring?: "wide";
} & ComponentPropsWithoutRef<"span">;

const COLOR_KEYWORDS: Record<string, string> = { inherit: "currentColor", primary: "var(--brand)" };

export default function HubSpinner({ color, label = "Loading", ring, className, ...rest }: HubSpinnerProps) {
  // Spinner takes one class attribute, so the class of spinner.css is named here beside the others.
  const classes = "spinner" + (ring === "wide" ? " " + styles.wide : "") + (className ? " " + className : "");
  return <Spinner color={color ? (COLOR_KEYWORDS[color] ?? color) : undefined} aria-label={label} className={classes} {...rest} />;
}
