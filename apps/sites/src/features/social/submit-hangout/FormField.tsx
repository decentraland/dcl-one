import { textClasses } from "@dcl/one-ui";
import { useId, type ReactNode } from "react";

import styles from "./FormField.module.css";

/**
 * A `WebSelect` or `WebTextarea` of the hangout form with its visible label, set in the label type of
 * `InputWebCreatorTools`, which draws its own: those two shared controls take no label.
 */
export default function FormField({ label, className, children }: {
  label: string;
  className?: string;
  /** The control, given the id its label points to. */
  children: (id: string) => ReactNode;
}) {
  const id = useId();
  return (
    <div className={className ? `${styles.root} ${className}` : styles.root}>
      <label htmlFor={id} className={textClasses.bodyMdStrong}>{label}</label>
      {children(id)}
    </div>
  );
}
