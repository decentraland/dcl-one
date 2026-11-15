import styles from './StatusSnackbar.module.css';

/**
 * An EARN snackbar in place of the landing CTAs: an icon and a message, then its actions.
 * @param props.tone - `danger` paints System/Red Dark 2, `neutral` Charcoal.
 * @param props.role - `status` for a standing state, `alert` for a failure.
 * @param props.children - {@link SnackbarAction}s.
 */
export function StatusSnackbar({
  tone,
  role,
  icon,
  message,
  children,
}: {
  tone: 'danger' | 'neutral';
  role: 'status' | 'alert';
  icon: React.ReactNode;
  message: string;
  children: React.ReactNode;
}) {
  return (
    <div
      className={styles.snackbar}
      data-tone={tone}
      role={role}
    >
      <p className={styles.message}>
        {icon}
        {message}
      </p>
      {children}
    </div>
  );
}

/** A text action in a {@link StatusSnackbar}, in Snow, White 60 (`muted`) or Primary. */
export function SnackbarAction({
  emphasis = 'default',
  onClick,
  children,
}: {
  emphasis?: 'default' | 'muted' | 'primary';
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      className={styles.action}
      type="button"
      data-emphasis={emphasis}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
