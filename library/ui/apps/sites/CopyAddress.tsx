import { CopyFeedback } from '../../index'
import { GeneralCheck, GeneralCopy } from '../../foundations/icons'

import styles from './CopyAddress.module.css'

export function CopyAddress({ address }: { address: string }) {
  return (
    <CopyFeedback
      value={address}
      showTooltip={false}
      durationMs={2000}
      successAnnouncement="Address copied!"
      errorAnnouncement="Couldn't copy the address"
    >
      {(copy, state) => (
        <>
          <button type="button" className={styles.copy} aria-label="Copy address" onClick={copy}>
            <GeneralCopy size={16} aria-hidden="true" />
          </button>
          {state !== 'idle' ? (
            <span className={styles.copied} aria-hidden="true">
              {state === 'copied' ? (
                <>
                  <GeneralCheck size={14} aria-hidden="true" /> Address copied!
                </>
              ) : (
                "Couldn't copy the address"
              )}
            </span>
          ) : null}
        </>
      )}
    </CopyFeedback>
  )
}
