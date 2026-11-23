import styles from './NotificationDot.module.css';

/** The unread dot at the top-right corner of its nearest positioned ancestor; the caller names the state for screen readers. */
export function NotificationDot() {
  return (
    <span
      className={styles.dot}
      aria-hidden="true"
    />
  );
}
