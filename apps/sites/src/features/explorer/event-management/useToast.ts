import { useEffect, useState } from "react";

/** As the source's snackbar: the outcome of an admin action shows for a few seconds. */
export const TOAST_MS = 4000;

export type Toast = { message: string; tone: "success" | "error" };

export function useToast() {
  const [toast, setToast] = useState<Toast | null>(null);
  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => setToast(null), TOAST_MS);
    return () => clearTimeout(timer);
  }, [toast]);
  return [toast, setToast] as const;
}
