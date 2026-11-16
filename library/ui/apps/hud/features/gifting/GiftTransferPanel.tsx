// The gift's transfer, drawn over the app by the host that signs it (the Sites /play web shell): "Preparing Gift for"
// the recipient with the item while the wallet signs (after ten seconds, a hint that it is taking longer, linking to
// the activity page), then "Gift Sent to <name>!" with rays and OK, or "Something went wrong" with TRY AGAIN and
// Contact Support. The host resolves the recipient and the item itself.

import { Avatar, Button, Modal, ModalActions, ModalTitle, Spinner } from '../../design'
import { ItemTile, RaysPopup } from '../rewards/RewardPopup'
import styles from './GiftTransferPanel.module.css'

export interface GiftTransferRecipient {
  address: string
  name?: string
  /** `#rgb`-style; drawn only as a colour. */
  nameColor?: string
  /** An https face snapshot. */
  face?: string
}

export interface GiftTransferItem {
  name: string
  image?: string
  rarity: string
  category?: string
}

export type GiftTransferView =
  | { step: 'loading' }
  | { step: 'preparing'; recipient: GiftTransferRecipient; item: GiftTransferItem; status: string; slow: boolean }
  | { step: 'sent'; recipient: GiftTransferRecipient }
  | { step: 'failed' }

export interface GiftTransferHandlers {
  onClose: () => void
  onRetry: () => void
  /** The slow hint's link: where the gift's delivery shows. */
  onActivity: () => void
  onSupport: () => void
}

export const GIFT_STATUS_WEB3 = 'A browser window should open for you to confirm the transaction.'
export const GIFT_STATUS_MANAGED = 'A window should open for you to confirm the transaction.'

const shortAddress = (a: string): string => `${a.slice(0, 6)}…${a.slice(-4)}`
const nameOf = (r: GiftTransferRecipient): string => r.name || shortAddress(r.address)

function Recipient({ recipient, size }: { recipient: GiftTransferRecipient; size: number }): React.JSX.Element {
  return <Avatar src={recipient.face} name={nameOf(recipient)} color={recipient.nameColor} size={size} />
}

export function GiftTransferPanel({ view, handlers }: { view: GiftTransferView; handlers: GiftTransferHandlers }): React.JSX.Element {
  switch (view.step) {
    case 'loading':
      return (
        <Modal width={420} ariaLabel="Preparing Gift">
          <div className={styles.centre}>
            <Spinner />
          </div>
        </Modal>
      )
    case 'preparing':
      return (
        <Modal width={480} ariaLabel={`Preparing Gift for ${nameOf(view.recipient)}`}>
          <ModalTitle title="Preparing Gift for" onClose={handlers.onClose} closeLabel="Close" />
          <div className={styles.body}>
            <div className={styles.recipient}>
              <Recipient recipient={view.recipient} size={32} />
              <span className={styles.name} style={view.recipient.nameColor ? { color: view.recipient.nameColor } : undefined}>
                {nameOf(view.recipient)}
              </span>
            </div>
            <ItemTile rarity={view.item.rarity} image={view.item.image} category={view.item.category} />
            <span className={styles.itemName}>{view.item.name}</span>
            <div className={styles.status} role="status">
              <Spinner size={18} />
              <span>{view.status}</span>
            </div>
            {view.slow && (
              <p className={styles.hint}>
                Sending your gift is taking longer than expected. You can close this window and check delivery status{' '}
                <button type="button" className={styles.link} onClick={handlers.onActivity}>
                  here
                </button>
                .
              </p>
            )}
          </div>
        </Modal>
      )
    case 'sent':
      return (
        <div className={styles.sent} onClick={handlers.onClose}>
          <RaysPopup labelledBy="gift-sent-title">
            <Recipient recipient={view.recipient} size={96} />
            <h2 id="gift-sent-title" className={styles.sentTitle}>
              Gift Sent to <span style={view.recipient.nameColor ? { color: view.recipient.nameColor } : undefined}>{nameOf(view.recipient)}</span>!
            </h2>
            <Button size="lg" onClick={handlers.onClose}>
              OK
            </Button>
          </RaysPopup>
        </div>
      )
    case 'failed':
      return (
        <Modal width={420} ariaLabel="Something went wrong">
          <ModalTitle title="Something went wrong" centered />
          <p className={styles.failed}>
            Your gift wasn&apos;t delivered. Please try again or contact Support.{' '}
            <button type="button" className={styles.link} onClick={handlers.onSupport}>
              Contact Support
            </button>
          </p>
          <ModalActions>
            <Button variant="secondary" onClick={handlers.onClose}>
              CLOSE
            </Button>
            <Button onClick={handlers.onRetry}>TRY AGAIN</Button>
          </ModalActions>
        </Modal>
      )
  }
}
