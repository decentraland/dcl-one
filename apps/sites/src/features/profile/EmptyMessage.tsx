import type { ReactNode } from "react";

import styles from "./EmptyMessage.module.css";

/** The icon frame, title, text and next step shown when a list has nothing in it. */
export function EmptyMessage({
  icon,
  title,
  subtitle,
  action,
}: {
  icon: ReactNode;
  title: string;
  subtitle: string;
  action: ReactNode;
}) {
  return (
    <div className={styles.empty}>
      <span className={styles.emptyIcon} aria-hidden="true">
        {icon}
      </span>
      <div className={styles.emptyBody}>
        <p className={styles.emptyTitle}>{title}</p>
        <p className={styles.emptySubtitle}>{subtitle}</p>
        <div className={styles.emptyAction}>{action}</div>
      </div>
    </div>
  );
}
