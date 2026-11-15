import type { AriaRole, HTMLAttributes, ReactNode } from "react";
import { forwardRef } from "react";
import styles from "./Alert.module.css";

export type AlertSeverity = "info" | "success" | "warning" | "error";

export type AlertProps = Omit<HTMLAttributes<HTMLDivElement>, "title" | "role"> & {
  severity?: AlertSeverity;
  variant?: "standard" | "filled" | "outlined";
  title?: ReactNode;
  /** Replaces the glyph of the severity; `false` shows none. */
  icon?: ReactNode | false;
  /** Controls at the end of the message; they replace the close button. */
  action?: ReactNode;
  /** Shows a close button. */
  onClose?: () => void;
  closeLabel?: string;
  /** Replaces the role of the severity: `alert` for error and warning, `status` for info and success. */
  role?: AriaRole;
};

const glyph = (path: ReactNode) => (
  <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" focusable="false">
    {path}
  </svg>
);

const GLYPHS: Record<AlertSeverity, ReactNode> = {
  info: glyph(<><circle cx="12" cy="12" r="9" /><path d="M12 11v6M12 7.5v.01" /></>),
  success: glyph(<><circle cx="12" cy="12" r="9" /><path d="M8 12.5l3 3 5-6" /></>),
  warning: glyph(<><path d="M12 4l9 16H3z" /><path d="M12 10v4M12 17v.01" /></>),
  error: glyph(<><circle cx="12" cy="12" r="9" /><path d="M12 7v6M12 16.5v.01" /></>),
};

/** A message with a severity. Errors and warnings interrupt a screen reader; the other two wait. */
const Alert = forwardRef<HTMLDivElement, AlertProps>(function Alert({
  severity = "info", variant = "standard", title, icon, action, onClose, closeLabel = "Close", role, className, children, ...rest
}, ref) {
  return (
    <div
      {...rest} ref={ref}
      role={role ?? (severity === "error" || severity === "warning" ? "alert" : "status")}
      className={styles.root + (className ? " " + className : "")}
      data-severity={severity} data-variant={variant}
    >
      {icon === false ? null : <span className={styles.icon} aria-hidden="true">{icon ?? GLYPHS[severity]}</span>}
      <div className={styles.message}>
        {title !== undefined ? <div className={styles.title}>{title}</div> : null}
        {children}
      </div>
      {action !== undefined ? <div className={styles.action}>{action}</div> : onClose ? (
        <div className={styles.action}>
          <button type="button" className={styles.close} aria-label={closeLabel} onClick={onClose}>
            <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true" focusable="false">
              <path d="M6 6l12 12M18 6L6 18" />
            </svg>
          </button>
        </div>
      ) : null}
    </div>
  );
});

export default Alert;
