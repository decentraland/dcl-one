import type { RefObject } from "react";
import { useEffect } from "react";

export function useDialogKeys(
  paperRef: RefObject<HTMLElement | null>,
  onClose?: () => void,
  enabled: boolean = true,
) {
  useEffect(() => {
    if (!enabled) return;
    const prev = document.activeElement;
    const paper = paperRef.current;
    paper && paper.focus();
    if (onClose) paper?.setAttribute("data-owns-escape", "");
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") { onClose && onClose(); return; }
      if (e.key !== "Tab" || !paperRef.current) return;
      const f = paperRef.current.querySelectorAll<HTMLElement>(
        'a[href], button:not([disabled]), input:not([disabled]), select, textarea, [tabindex]:not([tabindex="-1"])'
      );
      if (!f.length) { e.preventDefault(); paperRef.current.focus(); return; }
      const first = f[0], last = f[f.length - 1];
      if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last?.focus(); }
      else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first?.focus(); }
    }
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      paper?.removeAttribute("data-owns-escape");
      if (prev instanceof HTMLElement) prev.focus();
    };
  }, [paperRef, onClose, enabled]);
}
