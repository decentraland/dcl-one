import type { ReactNode } from "react";
import styles from "./Collapse.module.css";

export type CollapseProps = {
  open: boolean;
  // The trigger points at this id with `aria-controls` and owns `aria-expanded`.
  id?: string;
  className?: string;
  children?: ReactNode;
};

export default function Collapse({ open, id, className, children }: CollapseProps) {
  return (
    <div id={id} className={className ? styles.root + " " + className : styles.root} hidden={!open}>
      <div className={styles.inner}>{children}</div>
    </div>
  );
}
