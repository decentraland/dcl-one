import type { HTMLAttributes } from "react";
import { createElement } from "react";
import styles from "./Text.module.css";

export type TextVariant =
  | "h3"
  | "h4"
  | "h5"
  | "h6"
  | "subtitle1"
  | "subtitle2"
  | "body1"
  | "body2"
  | "caption"
  | "overline";

export type TextElement =
  | "h1"
  | "h2"
  | "h3"
  | "h4"
  | "h5"
  | "h6"
  | "p"
  | "span"
  | "div"
  | "strong"
  | "em"
  | "li"
  | "dt"
  | "dd"
  | "legend"
  | "figcaption";

export type TextProps = Omit<HTMLAttributes<HTMLElement>, "color"> & {
  variant?: TextVariant;
  as?: TextElement;
  tone?: "inherit" | "primary" | "secondary" | "error" | "disabled";
  align?: "left" | "center" | "right";
  noWrap?: boolean;
};

// The variant sets the look only. A heading level other than the default comes from `as`.
const DEFAULT_ELEMENT: Record<TextVariant, TextElement> = {
  h3: "h3",
  h4: "h4",
  h5: "h5",
  h6: "h6",
  subtitle1: "p",
  subtitle2: "p",
  body1: "p",
  body2: "p",
  caption: "span",
  overline: "span",
};

export default function Text({
  variant = "body1",
  as,
  tone = "inherit",
  align,
  noWrap = false,
  className,
  children,
  ...rest
}: TextProps) {
  return createElement(
    as ?? DEFAULT_ELEMENT[variant],
    {
      ...rest,
      className: className ? styles.root + " " + className : styles.root,
      "data-variant": variant,
      "data-tone": tone === "inherit" ? undefined : tone,
      "data-align": align,
      "data-nowrap": noWrap ? "" : undefined,
    },
    children,
  );
}
