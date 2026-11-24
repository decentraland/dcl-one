// A buy dialog's page reads its data again once a purchase went out, but only when the dialog is gone: a read empties
// the page until it answers, and would take the dialog and its notice with it. The dialog's outcome arms it; its close
// fires it, and so does the page unmounting first (browser Back on the notice), so the refresh is never lost. An
// outcome that arrives after the page unmounted (Back while the sale's receipt was awaited) reads at once, once.

import { useCallback, useEffect, useRef } from "react";

export function useReadOnClose(read: () => void): { arm(): void; fire(): void } {
  const armed = useRef(false);
  const mounted = useRef(false);
  const readAfterUnmount = useRef(false);
  const latest = useRef(read);
  latest.current = read;
  const fire = useCallback(() => {
    if (!armed.current) return;
    armed.current = false;
    latest.current();
  }, []);
  const arm = useCallback(() => {
    if (mounted.current) {
      armed.current = true;
      return;
    }
    // No notice left to keep up, and no close or unmount still to come: read now, and only once.
    if (readAfterUnmount.current) return;
    readAfterUnmount.current = true;
    latest.current();
  }, []);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      fire();
    };
  }, [fire]);
  return { arm, fire };
}
