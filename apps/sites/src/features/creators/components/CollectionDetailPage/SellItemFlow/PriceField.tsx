import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from '@/data/lib/creators/intl'
import { useFeatureFlag } from '@/features/creators/hooks/useFeatureFlag'
import { useManaUsdRate } from '@/features/creators/hooks/useSales'
import { FeatureFlag } from '@/data/lib/creators/featureFlags'
import { formatCredits, formatMana } from '@/data/lib/creators/publishFee'
import {
  MAX_SALE_CREDITS,
  MAX_SALE_MANA_WEI,
  MIN_SALE_MANA_WEI,
  formatCreditsAsUsd,
  formatManaAsUsd,
  parseManaAmount,
  sanitizeManaInput,
  type PriceCurrency
} from '@/data/lib/creators/sales'
import { CurrencyAmount } from '@/features/creators/components/CurrencyAmount'
import { Select, type SelectOption } from '@/features/creators/components/Select'
import styles from './SellItemModal.module.css'

export type PriceFormValues = {
  currency: PriceCurrency
  /** As typed: whole credits, or MANA with up to two decimals. */
  amount: string
}

type Props = {
  label: string
  values: PriceFormValues
  onChange: (values: PriceFormValues) => void
  /** A giveaway: the amount is frozen at 0 and the currency can't be picked. */
  free?: boolean
  disabled?: boolean
  /** Prefix of the field's test ids: `-input`, `-usd`, `-rate`, `-currency` (`-currency-glyph` when
   * there is only one currency to price in), `-error`. */
  testId: string
}

const ONE_MANA = 10n ** 18n

/**
 * Amount input whose leading currency glyph is the picker (credits or MANA), with a USD equivalent:
 * fixed for credits, estimated from the marketplace's MANA/USD oracle for MANA. Rendered inside the caller's field.
 */
export function PriceField({ label, values, onChange, free = false, disabled = false, testId }: Props) {
  const { t } = useTranslation()
  const { currency, amount } = values
  const rate = useManaUsdRate(currency === 'mana' && !free)
  const creditsFlag = useFeatureFlag(FeatureFlag.CREDITS_PRIMARY_LISTINGS)
  // Credits stay offered until the flag actually says otherwise: a form that flipped to MANA while the
  // flag was still being read would clear the amount the creator had already typed.
  const creditsEnabled = creditsFlag.isLoading || creditsFlag.enabled

  const options = useMemo<SelectOption<PriceCurrency>[]>(
    () => [
      ...(creditsEnabled
        ? [
            {
              value: 'credits' as const,
              label: t('sell_item_modal.price.currency_credits'),
              icon: <CurrencyAmount currency="credits">{null}</CurrencyAmount>
            }
          ]
        : []),
      {
        value: 'mana',
        label: t('sell_item_modal.price.currency_mana'),
        icon: <CurrencyAmount currency="mana">{null}</CurrencyAmount>
      }
    ],
    [t, creditsEnabled]
  )

  // The default currency is credits: with them off, a form that opened on credits moves to MANA.
  useEffect(() => {
    if (!creditsEnabled && currency === 'credits') onChange({ currency: 'mana', amount: '' })
  }, [creditsEnabled, currency, onChange])

  // Set when an input past the ceiling was dropped, so the creator learns why the field didn't take it.
  const [hitCeiling, setHitCeiling] = useState(false)

  const credits = Number(amount) || 0
  const manaWei = useMemo(() => parseManaAmount(amount), [amount])

  let error: string | null = null
  if (!free && currency === 'mana' && manaWei !== null && manaWei < MIN_SALE_MANA_WEI) {
    error = t('sell_item_modal.price.too_low_mana', { min: formatMana(MIN_SALE_MANA_WEI) })
  } else if (!free && hitCeiling) {
    error =
      currency === 'credits'
        ? t('sell_item_modal.price.too_high', { max: formatCredits(Number(MAX_SALE_CREDITS)) })
        : t('sell_item_modal.price.too_high_mana', { max: formatMana(MAX_SALE_MANA_WEI) })
  }

  let usd: string
  if (free) usd = formatCreditsAsUsd(0)
  else if (currency === 'credits') usd = formatCreditsAsUsd(credits)
  else usd = rate.data !== undefined && manaWei !== null ? `≈ ${formatManaAsUsd(manaWei, rate.data)}` : ''

  function changeAmount(value: string) {
    const next = currency === 'credits' ? value.replace(/\D/g, '') : sanitizeManaInput(value)
    // Input past the ceiling is dropped whole (never truncated into a different price), so the field and
    // its USD equivalent never outgrow the box.
    const tooHigh =
      currency === 'credits'
        ? Number(next) > Number(MAX_SALE_CREDITS)
        : (parseManaAmount(next) ?? 0n) > MAX_SALE_MANA_WEI
    setHitCeiling(tooHigh)
    if (!tooHigh) onChange({ currency, amount: next })
  }

  function changeCurrency(next: PriceCurrency) {
    // A typed amount means a different number in the other unit: never carry it over.
    if (next === currency) return
    setHitCeiling(false)
    onChange({ currency: next, amount: '' })
  }

  return (
    <>
      <span className={styles.label}>{label}</span>
      <div className={styles.box} data-disabled={free || undefined} data-invalid={error !== null || undefined}>
        {options.length === 1 ? (
          <span className={styles.currencyGlyph} data-testid={`${testId}-currency-glyph`}>
            <CurrencyAmount currency={options[0].value}>{null}</CurrencyAmount>
          </span>
        ) : (
          <Select
            value={currency}
            options={options}
            onChange={changeCurrency}
            variant="glyph"
            disabled={free || disabled}
            ariaLabel={t('sell_item_modal.price.currency_label')}
            testId={`${testId}-currency`}
          />
        )}
        <input
          type="text"
          inputMode={currency === 'credits' ? 'numeric' : 'decimal'}
          aria-label={label}
          placeholder="0"
          value={free ? '0' : amount}
          disabled={free || disabled}
          data-testid={`${testId}-input`}
          data-currency={currency}
          onChange={event => changeAmount(event.target.value)}
        />
        {usd && (
          <span className={styles.usd} data-testid={`${testId}-usd`}>
            {usd}
          </span>
        )}
      </div>
      <span className={styles.rate} data-testid={`${testId}-rate`}>
        {currency === 'credits' || free
          ? t('sell_item_modal.price.rate', { usd: formatCreditsAsUsd(1) })
          : rate.data !== undefined
            ? t('sell_item_modal.price.mana_rate', { usd: formatManaAsUsd(ONE_MANA, rate.data) })
            : t('sell_item_modal.price.mana_min', { min: formatMana(MIN_SALE_MANA_WEI) })}
      </span>
      {error && (
        <p className={styles.errorText} data-testid={`${testId}-error`}>
          {error}
        </p>
      )}
    </>
  )
}

export const DEFAULT_PRICE_VALUES: PriceFormValues = { currency: 'credits', amount: '' }
