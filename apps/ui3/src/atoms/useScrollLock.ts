import { useLayoutEffect } from "react";

let locks = 0;
let saved = { overflow: "", paddingRight: "" };

/**
 * While `locked`, the page behind does not scroll. Surfaces that lock at the same time share one lock:
 * the first one stores the body styles and the last one to go puts them back, in any order.
 */
export function useScrollLock(locked: boolean) {
  useLayoutEffect(() => {
    if (!locked) return;
    const { body } = document;
    if (locks === 0) {
      saved = { overflow: body.style.overflow, paddingRight: body.style.paddingRight };
      const gap = window.innerWidth - document.documentElement.clientWidth;
      body.style.overflow = "hidden";
      if (gap > 0) body.style.paddingRight = gap + "px";
    }
    locks += 1;
    return () => {
      locks -= 1;
      if (locks > 0) return;
      body.style.overflow = saved.overflow;
      body.style.paddingRight = saved.paddingRight;
    };
  }, [locked]);
}
