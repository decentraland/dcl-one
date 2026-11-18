import { CheckIcon } from '~/components/Icons'
import { useTranslation } from '~/intl'
import { Button } from '~/components/Button'
import { CurrencyAmount } from '~/components/CurrencyAmount'
import { openExternal } from '~/lib/navigation'
import { type PaymentMethod } from '~/lib/publishCollection'
import creditsMark from '~/assets/payment/credits-logo.webp'
import manaMark from '~/assets/payment/mana-logo.webp'
import { CheckboxBox } from '~/components/Checkbox'
import styles from './PaymentMethodCard.module.css'

const MARKS: Record<PaymentMethod, string> = { credits: creditsMark, mana: manaMark }

type Props = {
  method: PaymentMethod
  price: string
  balance: string
  /** Extra line under the price (e.g. the MANA-per-credit rate). */
  note?: string
  hasEnough: boolean
  getMoreUrl: string
  /** Buys the currency without leaving the wizard; the external link is the fallback when absent. */
  onGetMore?: () => void
  selected: boolean
  /** Hidden when it's the only method: a lone card is always the selection. */
  showCheckbox: boolean
  /** With several cards the buy button reads just "Buy"; alone it names the currency. */
  compactBuy: boolean
  disabled: boolean
  onSelect: () => void
}

export function PaymentMethodCard({
  method,
  price,
  balance,
  note,
  hasEnough,
  getMoreUrl,
  onGetMore,
  selected,
  showCheckbox,
  compactBuy,
  disabled,
  onSelect
}: Props) {
  const { t } = useTranslation()
  const isDisabled = disabled || !hasEnough
  // An auto-selected lone method with no balance must not look chosen.
  const isSelected = selected && hasEnough

  return (
    <label
      className={styles.card}
      data-testid={`payment-method-${method}`}
      data-method={method}
      data-selected={isSelected || undefined}
      data-disabled={isDisabled || undefined}
    >
      <input
        type="radio"
        name="payment-method"
        value={method}
        checked={isSelected}
        disabled={isDisabled}
        data-testid={`payment-method-${method}-input`}
        onChange={onSelect}
      />
      {showCheckbox && (
        <CheckboxBox aria-hidden data-checked={isSelected || undefined}>
          <CheckIcon />
        </CheckboxBox>
      )}
      <img className={styles.mark} src={MARKS[method]} alt="" />
      <span className={styles.info}>
        <span className={styles.title}>{t(`publish_collection_modal.payment_step.${method}`)}</span>
        <span
          className={styles.balance}
          data-insufficient={hasEnough ? undefined : true}
          data-testid={`payment-method-${method}-balance`}
        >
          {t(`publish_collection_modal.payment_step.${method}_balance`)}
          <span>
            <CurrencyAmount currency={method}>{balance}</CurrencyAmount>
          </span>
        </span>
      </span>
      <span className={styles.price}>
        <span className={styles.amount} data-testid={`payment-method-${method}-price`}>
          <CurrencyAmount currency={method}>{price}</CurrencyAmount>
        </span>
        {note && <span className={styles.rate}>{note}</span>}
      </span>
      {!hasEnough && (
        <Button
          {...(onGetMore ? { type: 'button' as const } : { as: 'a' as const, href: getMoreUrl })}
          variant="gradient"
          size="sm"
          disabled={disabled}
          data-testid={`payment-method-${method}-buy`}
          onClick={(event: React.MouseEvent) => {
            event.preventDefault()
            if (onGetMore) onGetMore()
            else openExternal(getMoreUrl)
          }}
        >
          <CurrencyAmount currency={method}>
            {t(
              compactBuy
                ? 'publish_collection_modal.payment_step.buy'
                : `publish_collection_modal.payment_step.buy_${method}`
            )}
          </CurrencyAmount>
        </Button>
      )}
    </label>
  )
}
