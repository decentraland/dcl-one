import { useMemo, useState, type FormEvent } from 'react'
import {
  CalendarTodayOutlinedIcon as CalendarIcon,
  InfoOutlinedIcon as InfoIcon,
  WarningAmberOutlinedIcon as WarningIcon
} from '~/components/Icons'
import DatePicker from 'react-datepicker'
import { enUS, es, zhCN, type Locale as DateLocale } from 'date-fns/locale'
import { useIntl } from 'react-intl'
import 'react-datepicker/dist/react-datepicker.css'
import { useTranslation } from '~/intl'
import { type Locale } from '~/store/locale'
import { useFriends } from '~/hooks/useSales'
import { type Session } from '~/lib/auth'
import { type Item } from '~/lib/items'
import {
  NO_EXPIRATION,
  formatDateValue,
  isValidAddress,
  minExpirationDate,
  parseExpirationDate,
  toPricedSale,
  type SalePrice
} from '~/lib/sales'
import { Button } from '~/components/Button'
import { Checkbox } from '~/components/Checkbox'
import { Modal } from '~/components/Modal'
import { InfoTooltip } from '~/components/Tooltip'
import { BeneficiaryInput } from './BeneficiaryInput'
import { DEFAULT_PRICE_VALUES, PriceField, type PriceFormValues } from './PriceField'
import { SellItemCard } from './SellItemCard'
import { Switch } from '~/components/Switch'
import { cx } from '~/lib/cx'
import styles from './SellItemModal.module.css'

const DATE_LOCALES: Record<Locale, DateLocale> = { en: enUS, es, zh: zhCN }

export type SellFormValues = {
  selfBeneficiary: boolean
  beneficiary: string
  price: PriceFormValues
  free: boolean
  withExpiration: boolean
  /** `YYYY-MM-DD`, as the native date input reports it. */
  expirationDate: string
}

export const DEFAULT_SELL_VALUES: SellFormValues = {
  selfBeneficiary: true,
  beneficiary: '',
  price: DEFAULT_PRICE_VALUES,
  free: false,
  withExpiration: false,
  expirationDate: ''
}

export type SellSubmission = {
  price: SalePrice
  beneficiary: string
  expiresAt: number
}

type Props = {
  item: Item
  session: Session
  /** Restores a previous attempt's form after a failure. */
  initialValues?: SellFormValues
  /** Warns that buyers get the last approved version while edits await the committee. */
  hasPendingChanges?: boolean
  /** A submit is in flight (custodial wallet, no prompt to wait for): the form dims and the button spins. */
  busy: boolean
  onSubmit: (values: SellFormValues, submission: SellSubmission) => void
  onClose: () => void
}

/** Resolves the form into what gets signed, or null while something is still missing or invalid. */
export function toSubmission(values: SellFormValues, address: string, now = Date.now()): SellSubmission | null {
  const beneficiary = values.selfBeneficiary ? address : values.beneficiary
  if (!values.free && !isValidAddress(beneficiary)) return null
  const price = toPricedSale(values.price.currency, values.price.amount)
  if (!values.free && price === null) return null
  let expiresAt = NO_EXPIRATION
  if (values.withExpiration) {
    const parsed = parseExpirationDate(values.expirationDate)
    if (parsed === null || parsed <= now) return null
    expiresAt = parsed
  }
  return {
    price: values.free || price === null ? { kind: 'free' } : price,
    beneficiary: values.free ? address : beneficiary,
    expiresAt
  }
}

/** The Sell Item form: beneficiary, price in credits or MANA (or giveaway), optional expiration date. */
export function SellItemModal({
  item,
  session,
  initialValues = DEFAULT_SELL_VALUES,
  hasPendingChanges = false,
  busy,
  onSubmit,
  onClose
}: Props) {
  const { t } = useTranslation()
  const intl = useIntl()
  const [values, setValues] = useState<SellFormValues>(initialValues)
  const friends = useFriends(session, !values.selfBeneficiary)
  const minDate = useMemo(() => minExpirationDate(), [])

  const submission = useMemo(() => toSubmission(values, session.address), [values, session.address])
  const canSubmit = submission !== null && !busy

  const expirationDate = useMemo(() => {
    const parsed = parseExpirationDate(values.expirationDate)
    return parsed === null ? null : new Date(parsed)
  }, [values.expirationDate])

  const noteCurrency = values.price.currency

  function update(patch: Partial<SellFormValues>) {
    setValues(current => ({ ...current, ...patch }))
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault()
    if (busy) return
    // Re-derived at submit time: the memoized check may hold a stale `now` if the form sat open past midnight.
    const fresh = toSubmission(values, session.address)
    if (fresh) onSubmit(values, fresh)
  }

  return (
    <Modal
      title={t('sell_item_modal.title')}
      size="wide"
      compact
      onClose={onClose}
      closeDisabled={busy}
      testId="sell-item-modal"
    >
      <hr className={styles.divider} />
      <form className={styles.form} onSubmit={handleSubmit} data-testid="sell-item-form">
        <div
          className={styles.fields}
          data-busy={busy || undefined}
          aria-busy={busy || undefined}
          {...(busy ? { inert: '' } : {})}
        >
          <SellItemCard item={item} />
          {hasPendingChanges && (
            <div className={styles.note} data-variant="warning" data-testid="sell-pending-changes">
              <WarningIcon aria-hidden />
              <p>{t('sell_item_modal.pending_changes')}</p>
            </div>
          )}
          <h3 className={styles.subtitle}>{t('sell_item_modal.subtitle')}</h3>

          <div className={styles.field}>
            <span className={styles.label}>{t('sell_item_modal.beneficiary.label')}</span>
            <Checkbox
              checked={values.selfBeneficiary}
              onChange={checked => update({ selfBeneficiary: checked })}
              disabled={busy}
              testId="sell-self-beneficiary"
            >
              {t('sell_item_modal.beneficiary.self')}
            </Checkbox>
            {!values.selfBeneficiary && (
              <BeneficiaryInput
                value={values.beneficiary}
                onChange={beneficiary => update({ beneficiary })}
                friends={friends.data}
                isLoadingFriends={friends.isLoading}
                disabled={busy}
                testId="sell-beneficiary"
              />
            )}
          </div>

          <div className={styles.field}>
            <PriceField
              label={t('sell_item_modal.price.label')}
              values={values.price}
              onChange={price => update({ price })}
              free={values.free}
              disabled={busy}
              testId="sell-price"
            />
            <Checkbox checked={values.free} onChange={free => update({ free })} disabled={busy} testId="sell-free">
              {t('sell_item_modal.price.giveaway')}
            </Checkbox>
          </div>

          <div className={styles.field}>
            <div className={styles.row}>
              <span className={styles.label} id="sell-expiration-label">
                {t('sell_item_modal.expiration.label')}
              </span>
              <Switch
                checked={values.withExpiration}
                onChange={withExpiration => update({ withExpiration })}
                disabled={busy}
                label={t('sell_item_modal.expiration.label')}
                testId="sell-expiration-toggle"
              />
            </div>
            {values.withExpiration && (
              <div className={cx(styles.box, styles.dateField)}>
                <CalendarIcon aria-hidden />
                <DatePicker
                  selected={expirationDate}
                  onChange={(date: Date | null) => update({ expirationDate: date ? formatDateValue(date) : '' })}
                  minDate={minDate}
                  locale={DATE_LOCALES[intl.locale as Locale] ?? enUS}
                  dateFormat={t('sell_item_modal.expiration.date_format')}
                  placeholderText={t('sell_item_modal.expiration.date_placeholder')}
                  disabled={busy}
                  showPopperArrow={false}
                  popperProps={{ strategy: 'fixed' }}
                  popperPlacement="bottom-start"
                  ariaLabelledBy="sell-expiration-label"
                  customInput={<input data-testid="sell-expiration-date" />}
                />
              </div>
            )}
          </div>

          <div className={styles.note} data-testid="sell-note" data-currency={noteCurrency}>
            <InfoIcon aria-hidden />
            <p>
              {noteCurrency === 'mana'
                ? t('sell_item_modal.note_mana')
                : intl.formatMessage(
                    { id: 'sell_item_modal.note' },
                    {
                      b: chunks => (
                        <b>
                          {chunks}
                          <InfoTooltip content={t('sell_item_modal.note_tooltip')} testId="sell-note-tooltip" />
                        </b>
                      )
                    }
                  )}
            </p>
          </div>
        </div>

        <div className={styles.footer}>
          <Button type="button" variant="secondary" disabled={busy} data-testid="sell-cancel" onClick={onClose}>
            {t('sell_item_modal.cancel')}
          </Button>
          <Button type="submit" loading={busy} disabled={!canSubmit} data-testid="sell-submit">
            {t('sell_item_modal.submit')}
          </Button>
        </div>
      </form>
    </Modal>
  )
}
