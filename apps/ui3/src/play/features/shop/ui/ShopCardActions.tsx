// A Shop card's actions, shown on hover in place of its price row: trying it on (TAKE OFF or STOP while
// it is on), then BUY when it is bought in-world, else VIEW on the web Shop.

import { Button, MaskIcon } from '@hud/design'
import externalIcon from '../../../assets/shop/external.svg'
import { pressedWith, type PressedWith } from './pressedWith'
import styles from './ShopCardActions.module.css'

export function ShopCardActions({
  tryOn,
  buyInWorld,
  onBuy,
  onView
}: {
  /** Absent when the item can't be tried on. */
  tryOn?: { label: string; disabled?: boolean; title?: string; onClick: () => void }
  buyInWorld: boolean
  /** BUY, with how it was pressed. */
  onBuy: (input: PressedWith) => void
  /** Absent when the item has no page on the web Shop to view it on. */
  onView?: () => void
}): React.JSX.Element {
  return (
    <>
      {tryOn != null && (
        <Button variant="secondary" size="card" className={styles.action} disabled={tryOn.disabled} title={tryOn.title} onClick={tryOn.onClick}>
          {tryOn.label}
        </Button>
      )}
      {buyInWorld ? (
        <Button size="card" className={styles.action} onClick={(e) => onBuy(pressedWith(e))}>
          BUY
        </Button>
      ) : (
        onView != null && (
          <Button variant="secondary" size="card" className={styles.action} onClick={onView}>
            VIEW
            <MaskIcon src={externalIcon} size={20} />
          </Button>
        )
      )}
    </>
  )
}
