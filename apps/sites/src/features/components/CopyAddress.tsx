import { Fragment, useEffect, useRef, useState } from "react";

import { GeneralCheck, GeneralCopy } from "@dcl/one-ui/foundations/icons";

import styles from "./CopyAddress.module.css";

type CopyState = "idle" | "copied" | "failed";

/**
 * The live region stays mounted and only its text changes, so screen readers announce the result.
 * A denied or missing clipboard (an insecure page, a refused permission) says so instead of failing
 * silently.
 */
export function CopyAddress({ address }: { address: string }) {
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
  const copy = () => {
    const clipboard = typeof navigator === "undefined" ? undefined : navigator.clipboard;
    if (!clipboard) return show("failed");
    clipboard.writeText(address).then(
      () => show("copied"),
      () => show("failed"),
    );
  };
  return (
    <span className={styles.copyWrap}>
      <button type="button" className={styles.copy} aria-label="Copy address" onClick={copy}>
        <GeneralCopy size={16} aria-hidden="true" />
      </button>
      <span className={state === "idle" ? styles.copiedHidden : styles.copied} role="status">
        <Fragment key={announcement}>
          {state === "copied" ? (
            <>
              <GeneralCheck size={14} aria-hidden="true" /> Address copied!
            </>
          ) : state === "failed" ? (
            "Couldn't copy the address"
          ) : null}
        </Fragment>
      </span>
    </span>
  );
}
