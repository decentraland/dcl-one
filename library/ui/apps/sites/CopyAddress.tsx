import { CopyFeedback } from '../../index'
import { Button } from '../../components/Button/Button'
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
          <Button
            variant="text"
            iconOnly
            round={24}
            iconSize={16}
            aria-label="Copy address"
            onClick={copy}
          >
            <GeneralCopy aria-hidden="true" />
          </Button>
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
