import type { AriaRole, FocusEvent, KeyboardEvent, ReactNode } from "react";
import { createContext, useContext, useEffect, useRef, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import Alert, { type AlertProps, type AlertSeverity } from "./Alert";
import styles from "./Toast.module.css";

export type ToastCloseReason = "timeout" | "dismiss" | "escape";
export type ToastPlacement = "top-left" | "top-center" | "top-right" | "bottom-left" | "bottom-center" | "bottom-right";

export type ToastProps = {
  open?: boolean;
  onClose?: (reason: ToastCloseReason) => void;
  /** Milliseconds until the toast asks to close. `null` keeps it until it is dismissed. */
  duration?: number | null;
  /** The time to close also stops while the window is not the active one. */
  holdOnWindowBlur?: boolean;
  /** Milliseconds until the toast asks to close after a stop ends. With no value it goes on with the time that was left. */
  resumeDuration?: number;
  /** Draws the content as an `Alert` with a close button. With no severity the children draw their own surface. */
  severity?: AlertSeverity;
  variant?: AlertProps["variant"];
  title?: ReactNode;
  icon?: AlertProps["icon"];
  action?: ReactNode;
  /** Corner of the viewport. Not used inside a `ToastHost`, which places its toasts. */
  placement?: ToastPlacement;
  /** Replaces the role: `alert` for error and warning, `status` for the rest. */
  role?: AriaRole;
  closeLabel?: string;
  /** A new value starts the time to close again, as when the same toast is shown again. */
  restartKey?: unknown;
  className?: string;
  children?: ReactNode;
};

const InHost = createContext(false);

/** A short message over the page. The time to close stops while the pointer or focus is on the toast. */
export function Toast({
  open = true, onClose, duration = 5000, holdOnWindowBlur = false, resumeDuration, severity, variant, title, icon, action, placement = "bottom-left", role, closeLabel,
  restartKey, className, children,
}: ToastProps) {
  const inHost = useContext(InHost);
  const close = useRef(onClose);
  const resume = useRef(resumeDuration);
  useEffect(() => {
    close.current = onClose;
    resume.current = resumeDuration;
  });

  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const clock = useRef({ left: 0, since: 0, hover: false, focus: false, away: false });
  const run = () => {
    clearTimeout(timer.current);
    if (duration === null || clock.current.hover || clock.current.focus || clock.current.away) return;
    clock.current.since = Date.now();
    timer.current = setTimeout(() => close.current?.("timeout"), Math.max(clock.current.left, 0));
  };
  const hold = () => {
    if (timer.current === undefined) return;
    clearTimeout(timer.current);
    timer.current = undefined;
    clock.current.left -= Date.now() - clock.current.since;
  };
  const setHeld = (by: "hover" | "focus" | "away", held: boolean) => {
    if (clock.current[by] === held) return;
    if (held) hold();
    clock.current[by] = held;
    if (held) return;
    const free = !clock.current.hover && !clock.current.focus && !clock.current.away;
    if (free && resume.current !== undefined) clock.current.left = resume.current;
    run();
  };

  useEffect(() => {
    if (!open) return;
    clock.current.left = duration ?? 0;
    run();
    const onAway = () => setHeld("away", true);
    const onBack = () => setHeld("away", false);
    if (holdOnWindowBlur) {
      window.addEventListener("blur", onAway);
      window.addEventListener("focus", onBack);
    }
    return () => {
      clearTimeout(timer.current);
      timer.current = undefined;
      clock.current.away = false;
      window.removeEventListener("blur", onAway);
      window.removeEventListener("focus", onBack);
    };
  }, [open, duration, holdOnWindowBlur, restartKey]);

  // A closed toast leaves the page with the pointer and focus on it: no leave event comes, and the next open starts free.
  useEffect(() => {
    if (!open) return;
    return () => {
      clock.current.hover = false;
      clock.current.focus = false;
    };
  }, [open]);

  if (typeof document === "undefined" || (inHost && !open)) return null;

  const toast = open && (
    <div
      className={styles.toast + (className ? " " + className : "")}
      role={severity ? undefined : role ?? "status"}
      onMouseEnter={() => setHeld("hover", true)}
      onMouseLeave={() => setHeld("hover", false)}
      // Only an Escape in the toast is for the toast: an Escape elsewhere can close a tip or a menu.
      onKeyDown={(event: KeyboardEvent<HTMLDivElement>) => {
        if (event.key === "Escape" && !event.defaultPrevented) close.current?.("escape");
      }}
      onFocus={() => setHeld("focus", true)}
      onBlur={(event: FocusEvent<HTMLDivElement>) => {
        if (!(event.relatedTarget instanceof Node) || !event.currentTarget.contains(event.relatedTarget)) setHeld("focus", false);
      }}
    >
      {severity ? (
        <Alert
          severity={severity} variant={variant} title={title} icon={icon} action={action} role={role} closeLabel={closeLabel}
          className={styles.alert} onClose={onClose ? () => onClose("dismiss") : undefined}
        >
          {children}
        </Alert>
      ) : children}
    </div>
  );
  if (inHost) return toast;
  // The live region stays in the page while the toast is closed: a screen reader reads a message that comes into a
  // region it already knows, and can miss one that comes in with its region.
  return createPortal(<div className={styles.viewport} data-placement={placement} aria-live="polite">{toast}</div>, document.body);
}

export type ToastItem = Omit<ToastProps, "open" | "placement" | "restartKey"> & { id: string };

export type ToastStore = {
  /** Adds a toast to the stack. Returns its id. */
  show: (toast: Omit<ToastItem, "id"> & { id?: string }) => string;
  dismiss: (id: string) => void;
  get: () => readonly ToastItem[];
  subscribe: (listener: () => void) => () => void;
};

/** The list behind a `ToastHost`: code outside the React tree adds and removes toasts through it. */
export function createToastStore(): ToastStore {
  let items: readonly ToastItem[] = [];
  let count = 0;
  const listeners = new Set<() => void>();
  const set = (next: readonly ToastItem[]) => {
    items = next;
    for (const listener of listeners) listener();
  };
  return {
    show: (toast) => {
      const id = toast.id ?? "toast-" + ++count;
      set([...items.filter((item) => item.id !== id), { ...toast, id }]);
      return id;
    },
    dismiss: (id) => { if (items.some((item) => item.id === id)) set(items.filter((item) => item.id !== id)); },
    get: () => items,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
  };
}

const NO_ITEMS: readonly ToastItem[] = [];
const noItems = () => NO_ITEMS;
const noSubscription = () => () => {};

export type ToastHostProps = {
  placement?: ToastPlacement;
  /** Toasts of the store come after the children. A toast that asks to close leaves the store. */
  store?: ToastStore;
  className?: string;
  /** `Toast` elements the app renders itself. */
  children?: ReactNode;
};

/** The one stack of toasts of an app, in a portal on `document.body`. */
export function ToastHost({ placement = "bottom-left", store, className, children }: ToastHostProps) {
  const items = useSyncExternalStore(store?.subscribe ?? noSubscription, store?.get ?? noItems, store?.get ?? noItems);
  if (typeof document === "undefined") return null;
  return createPortal(
    <div className={styles.viewport + (className ? " " + className : "")} data-placement={placement} aria-live="polite">
      <InHost.Provider value={true}>
        {children}
        {/* Each `show` stores a new item: the same id shown again starts its time to close again. */}
        {items.map((item) => {
          const { id, onClose, ...toast } = item;
          return <Toast key={id} {...toast} restartKey={item} onClose={(reason) => { onClose?.(reason); store?.dismiss(id); }} />;
        })}
      </InHost.Provider>
    </div>,
    document.body,
  );
}

export default Toast;
