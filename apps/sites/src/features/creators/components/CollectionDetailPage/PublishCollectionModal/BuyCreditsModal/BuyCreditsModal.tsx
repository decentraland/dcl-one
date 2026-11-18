import { useMemo, useState } from 'react'
import { AddIcon, ChevronRightIcon, RemoveIcon, StarIcon } from '../../../Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { useCreditPacks } from '../../../../hooks/useCreditPacks'
import {
  MAX_PACK_QUANTITY,
  formatUsd,
  recommendPack,
  selectionTotals,
  type PackSelection,
  type PackTotals
} from '@/data/lib/creators/creditPacks'
import { formatCredits } from '@/data/lib/creators/publishFee'
import { Button } from '../../../Button'
import { CurrencyAmount } from '../../../CurrencyAmount'
import { Modal } from '../../../Modal'
import { artForPack } from '../packArt'
import { cx } from '@/data/lib/creators/cx'
import styles from './BuyCreditsModal.module.css'

// The catalogue has four packs; the placeholders keep the grid's shape while it loads.
const SKELETON_PACKS = 4

type Props = {
  balance: number
  /** Credits the creator is short of the fee; drives the preselected pack and quantity. */
  shortfall: number
  onCancel: () => void
  /** Starts the checkout; a rejection keeps the dialog open with an error line. */
  onBuy: (selection: PackSelection, totals: PackTotals) => Promise<void>
}

/**
 * The credit-pack picker stacked on the publish wizard: every pack, the recommended one preselected, and
 * the running total. While a single pack can cover the shortfall it is a plain one-pack picker; only when
 * none can does each pack grow a quantity stepper, since a checkout buys copies of a single pack.
 */
export function BuyCreditsModal({ balance, shortfall, onCancel, onBuy }: Props) {
  const { t } = useTranslation()
  const { packs: catalogue } = useCreditPacks()
  const packs = useMemo(() => catalogue ?? [], [catalogue])
  const recommended = useMemo(() => recommendPack(packs, shortfall), [packs, shortfall])
  const withQuantities = useMemo(() => !packs.some(pack => pack.credits >= shortfall), [packs, shortfall])
  const [chosen, setChosen] = useState<PackSelection | null>(null)
  const [isBuying, setBuying] = useState(false)
  const [failed, setFailed] = useState(false)
  const selection = chosen ?? recommended
  const totals = useMemo(() => selectionTotals(packs, selection), [packs, selection])
  const canBuy = totals.credits > 0 && !isBuying

  function setQuantity(packId: string, quantity: number) {
    setChosen(quantity > 0 ? { packId, quantity: Math.min(MAX_PACK_QUANTITY, quantity) } : { packId, quantity: 0 })
  }

  function quantityOf(packId: string): number {
    return selection?.packId === packId ? selection.quantity : 0
  }

  async function buy() {
    if (!selection || !canBuy) return
    setBuying(true)
    setFailed(false)
    try {
      await onBuy(selection, totals)
    } catch {
      setFailed(true)
      setBuying(false)
    }
  }

  return (
    <Modal
      title={t('publish_collection_modal.buy_credits.title')}
      size="large"
      hideTitle
      showClose
      closeDisabled={isBuying}
      onClose={onCancel}
      testId="buy-credits-modal"
    >
      <div className={styles.wrap}>
        <div className={styles.header}>
          <h3 className={styles.heading}>{t('publish_collection_modal.buy_credits.title')}</h3>
          <p className={styles.balance} data-testid="buy-credits-balance">
            {t('publish_collection_modal.buy_credits.balance')}
            <span>
              <CurrencyAmount currency="credits">{formatCredits(balance)}</CurrencyAmount>
            </span>
          </p>
        </div>

        <div className={styles.packs} data-testid="credit-packs" aria-busy={!catalogue || undefined}>
          {!catalogue &&
            Array.from({ length: SKELETON_PACKS }, (_, index) => (
              <div className={styles.pack} key={index} data-skeleton data-testid="credit-pack-skeleton" aria-hidden>
                <span className={styles.credits}>
                  <span className={cx(styles.skeletonLine, 'skeleton')} data-size="amount" />
                  <span className={cx(styles.skeletonLine, 'skeleton')} data-size="label" />
                </span>
                <span className={cx(styles.skeletonArt, 'skeleton')} />
                <span className={cx(styles.skeletonLine, 'skeleton')} data-size="price" />
              </div>
            ))}
          {packs.map((pack, index) => {
            const quantity = quantityOf(pack.id)
            return (
              <div
                className={styles.pack}
                key={pack.id}
                role="button"
                tabIndex={0}
                data-testid={`credit-pack-${pack.id}`}
                data-selected={quantity > 0 || undefined}
                data-disabled={isBuying || undefined}
                aria-pressed={quantity > 0}
                aria-label={t('publish_collection_modal.buy_credits.pack_label', {
                  credits: formatCredits(pack.credits),
                  usd: formatUsd(pack.usd)
                })}
                onClick={() => quantity === 0 && setQuantity(pack.id, 1)}
                onKeyDown={event => {
                  if ((event.key === 'Enter' || event.key === ' ') && quantity === 0) {
                    event.preventDefault()
                    setQuantity(pack.id, 1)
                  }
                }}
              >
                {recommended?.packId === pack.id && (
                  <span className={styles.badge} data-testid="credit-pack-recommended" aria-hidden>
                    <StarIcon />
                    {t('publish_collection_modal.buy_credits.recommended')}
                  </span>
                )}
                <span className={styles.credits}>
                  <span className={styles.creditsAmount}>
                    <CurrencyAmount currency="credits">{formatCredits(pack.credits)}</CurrencyAmount>
                  </span>
                  <span className={styles.creditsLabel}>{t('publish_collection_modal.buy_credits.credits')}</span>
                </span>
                <img className={styles.art} src={artForPack(pack, index)} alt="" />
                <span className={styles.price}>{formatUsd(pack.usd)}</span>
                {withQuantities && (
                  <div
                    className={styles.stepper}
                    onClick={event => event.stopPropagation()}
                    onKeyDown={event => event.stopPropagation()}
                  >
                    <button
                      className={styles.stepButton}
                      type="button"
                      disabled={quantity === 0 || isBuying}
                      aria-label={t('publish_collection_modal.buy_credits.decrease', {
                        credits: formatCredits(pack.credits)
                      })}
                      data-testid={`credit-pack-${pack.id}-decrease`}
                      onClick={() => setQuantity(pack.id, quantity - 1)}
                    >
                      <RemoveIcon />
                    </button>
                    <span data-testid={`credit-pack-${pack.id}-quantity`}>{quantity}</span>
                    <button
                      className={styles.stepButton}
                      type="button"
                      disabled={quantity >= MAX_PACK_QUANTITY || isBuying}
                      aria-label={t('publish_collection_modal.buy_credits.increase', {
                        credits: formatCredits(pack.credits)
                      })}
                      data-testid={`credit-pack-${pack.id}-increase`}
                      onClick={() => setQuantity(pack.id, quantity + 1)}
                    >
                      <AddIcon />
                    </button>
                  </div>
                )}
              </div>
            )
          })}
        </div>

        <p className={styles.total} data-testid="buy-credits-total">
          {t('publish_collection_modal.buy_credits.total')}
          <span>
            <CurrencyAmount currency="credits">{formatCredits(totals.credits)}</CurrencyAmount>
          </span>
          <span data-testid="buy-credits-total-usd">{formatUsd(totals.usd)}</span>
        </p>
        {failed && (
          <p className={styles.errorText} data-testid="buy-credits-error">
            {t('publish_collection_modal.buy_credits.error')}
          </p>
        )}

        <div className={styles.footer}>
          <Button
            type="button"
            variant="secondary"
            disabled={isBuying}
            data-testid="buy-credits-cancel"
            onClick={onCancel}
          >
            {t('publish_collection_modal.buy_credits.cancel')}
          </Button>
          <Button
            type="button"
            loading={isBuying}
            disabled={!canBuy}
            data-testid="buy-credits-submit"
            onClick={() => void buy()}
          >
            {t('publish_collection_modal.buy_credits.buy', { credits: formatCredits(totals.credits) })}
            <ChevronRightIcon fontSize="small" />
          </Button>
        </div>
      </div>
    </Modal>
  )
}
