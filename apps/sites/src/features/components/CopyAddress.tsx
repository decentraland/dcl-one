import { Fragment } from "react";

import { GeneralCheck, GeneralCopy } from "@dcl/one-ui/foundations/icons";

import styles from "./CopyAddress.module.css";
import { useCopyStatus } from "./useCopyStatus";

/**
 * The live region stays mounted and only its text changes, so screen readers announce the result.
 * A denied or missing clipboard (an insecure page, a refused permission) says so instead of failing
 * silently.
 */
export function CopyAddress({ address }: { address: string }) {
  const { state, announcement, copy: copyText } = useCopyStatus();
  const copy = () => copyText(address);
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
