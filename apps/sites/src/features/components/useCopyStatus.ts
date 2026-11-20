import { useEffect, useRef, useState } from "react";

export type CopyState = "idle" | "copied" | "failed";

/**
 * Copies text and reports how it went for two seconds. `announcement` changes with every result, so
 * a live region keyed by it announces a repeated one. A denied or missing clipboard (an insecure
 * page, a refused permission) is `failed`, never silent.
 */
export function useCopyStatus(): { state: CopyState; announcement: number; copy: (text: string) => void } {
  const [state, setState] = useState<CopyState>("idle");
  const [announcement, setAnnouncement] = useState(0);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  const show = (next: CopyState) => {
    setState(next);
    setAnnouncement((current) => current + 1);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setState("idle"), 2000);
  };
  const copy = (text: string) => {
    const clipboard = typeof navigator === "undefined" ? undefined : navigator.clipboard;
    if (!clipboard) return show("failed");
    clipboard.writeText(text).then(
      () => show("copied"),
      () => show("failed"),
    );
  };
  return { state, announcement, copy };
}
