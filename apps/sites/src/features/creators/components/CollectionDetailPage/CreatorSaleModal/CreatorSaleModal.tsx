import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { ChevronLeftIcon, ChevronRightIcon, CloseIcon, InfoOutlinedIcon } from '~/components/Icons'
import { useIntl } from 'react-intl'
import { useTranslation, type Translate } from '~/intl'
import { isSocialLogin, type Session } from '~/lib/auth'
import { discountedManaWei } from '~/lib/collectionDiscount'
import {
  DiscountInputError,
  MAX_DISCOUNT_PCT,
  MIN_DISCOUNT_PCT,
  toDiscountError,
  validateDiscountTerms,
  type CollectionDiscount,
  type DiscountInputProblem
} from '~/lib/coupons'
import { track } from '~/lib/analytics'
import { openExternal } from '~/lib/navigation'
import { formatCredits, formatMana } from '~/lib/publishFee'
import { discountedPriceCredits, type SaleableCollection } from '~/lib/saleableCollection'
import { startOfDay, windowOf, type SaleWhen } from '~/lib/saleWindow'
import { shopCreatorUrl } from '~/lib/shop'
import { useNotifications } from '~/lib/notifications'
import { Button } from '~/components/Button'
import { CollectionMosaic } from '~/components/CollectionMosaic'
import { ConfirmModal } from '~/components/ConfirmModal'
import { PendingModal } from '~/components/PendingModal'
import { StepIndicator } from '~/components/StepIndicator'
import { SuccessModal } from '~/components/SuccessModal'
import { Tooltip } from '~/components/Tooltip'
import { SaleTag } from '../SaleTag'
import { CurrencyIcon } from './CurrencyIcon'
import { ErrorNotice } from './ErrorNotice'
import { formatDateTime } from './dates'
import { RangePicker, type RangePickerHandle } from './RangePicker'
import { SaleCountdown } from './SaleCountdown'
import { SaleIcon as Icon } from './SaleIcon'
import modal from '~/components/Modal/Modal.module.css'
import styles from './CreatorSaleModal.module.css'

const PCT_PRESETS = [10, 20, 30, 50]
const DURATION_PRESETS = [
  { key: '24h', hours: 24 },
  { key: '3d', hours: 72 },
  { key: '7d', hours: 168 },
  { key: '14d', hours: 336 }
] as const
const HOUR_MS = 60 * 60 * 1000
const DAY_MS = 24 * HOUR_MS
/** The longest a sale can run, so the calendar never offers a day the terms would then refuse. */
const MAX_DAYS = 30
const WHEN_TRIGGER = '[data-sale-when-trigger]'
const DISCOUNT_REVIEWED_EVENT = 'Review collection discount'
const DISCOUNT_ABANDONED_EVENT = 'Abandon collection discount'
/** Roughly what the calendar needs under its trigger, presets stacked above the month on a phone. */
const CALENDAR_ROOM = 460

/** When the sale runs: a length from now, or two calendar days picked on the Shop's calendar. */
type When = SaleWhen<(typeof DURATION_PRESETS)[number]['key']>

function durationLabel(t: Translate, hours: number): string {
  return hours % 24 === 0 && hours >= 48
    ? t('creator_sale.duration_days', { count: hours / 24 })
    : t('creator_sale.duration_hours', { count: hours })
}

function problemCopy(t: Translate, problem: DiscountInputProblem): string {
  switch (problem) {
    case 'pct':
      return t('creator_sale.error_pct', { min: MIN_DISCOUNT_PCT, max: MAX_DISCOUNT_PCT })
    case 'duration':
      return t('creator_sale.error_duration', { days: MAX_DAYS })
    case 'collections':
      return t('creator_sale.error_collections')
    case 'uses':
      return t('creator_sale.error_uses')
    default:
      return t('creator_sale.error_window')
  }
}

/** What the modal signs: the creator's terms. The flow it is part of signs, stores and settles them. */
export type SaleTerms = {
  discountPct: number
  startsAtMs: number | undefined
  endsAtMs: number
  uses: number | undefined
}

/**
 * The Shop's "Discount this collection" modal, the same in the builder: pick a discount, when it runs and an
 * optional cap, review it, sign it. `onSubmit` signs and stores; `onUnconfirmed` takes over when the coupon was
 * signed but its storing got no answer, which only the flow around this modal can settle.
 */
export function CreatorSaleModal({
  session,
  collection,
  onSubmit,
  onUnconfirmed,
  onClose: closeModal
}: {
  session: Session
  collection: SaleableCollection
  onSubmit: (terms: SaleTerms, onSigned: () => void) => Promise<CollectionDiscount>
  onUnconfirmed: () => void
  onClose: () => void
}) {
  const { t, rich: tNode } = useTranslation()
  const intl = useIntl()
  const showToast = useNotifications(state => state.showToast)
  const current = collection
  const selected = useMemo(() => [current.contractAddress], [current])
  // The terms are agreed on the first step and confirmed on the second; nothing is signed until 'review'.
  const [step, setStep] = useState<'form' | 'review'>('form')
  const [pctPreset, setPctPreset] = useState<number | 'custom'>(20)
  const [customPct, setCustomPct] = useState('15')
  const [when, setWhen] = useState<When>({ kind: 'preset', key: '3d', hours: 72 })
  const [whenOpen, setWhenOpen] = useState(false)
  // Whether the calendar was open when the current backdrop press began.
  const calendarOpenOnPress = useRef(false)
  // The window and cap as the review showed them, fixed on the way in so the signature is exactly what was read.
  const [reviewed, setReviewed] = useState<{
    startsAtMs: number | undefined
    endsAtMs: number
    uses: number | undefined
  } | null>(null)
  // The review step was reached at least once this opening: the funnel counts it once, not on every Continue.
  const reviewTracked = useRef(false)
  // Ticks once a minute, so terms measured from now (the start hint, the 30-day bound) don't go stale on an open form.
  const [clock, setClock] = useState(() => Date.now())
  useEffect(() => {
    const timer = setInterval(() => setClock(Date.now()), 60_000)
    return () => clearInterval(timer)
  }, [])
  const strip = useRef<HTMLDivElement>(null)
  const [stripEdges, setStripEdges] = useState({ start: true, end: true })
  const whenPicker = useRef<RangePickerHandle>(null)
  const card = useRef<HTMLDivElement>(null)
  const body = useRef<HTMLDivElement>(null)
  const whenTrigger = useRef<HTMLButtonElement>(null)
  // Where the calendar floats: over the page, on the trigger's box, so opening it never grows the card.
  const [whenAnchor, setWhenAnchor] = useState<{ top: number; left: number; width: number; height: number } | null>(
    null
  )

  const anchor = useRef<HTMLDivElement>(null)
  // Leaving the terms unmounts the calendar mid-fold, before it can report itself closed.
  useEffect(() => {
    if (step === 'form') return
    setWhenOpen(false)
    setWhenAnchor(null)
  }, [step])
  // A short screen can leave no room under the trigger even after the lift: raise it just enough to fit.
  // Measured from the panel's own height: its position is still mid-grow here, shifted by the animation.
  useLayoutEffect(() => {
    if (!whenOpen) return
    const panel = anchor.current?.firstElementChild as HTMLElement | null | undefined
    if (!panel) return
    setWhenAnchor(at => {
      if (!at) return at
      const over = at.top + at.height + 8 + panel.offsetHeight - (window.innerHeight - 8)
      // Never past the top edge either: a landscape phone cannot fit it, and the panel scrolls instead.
      return over > 0 ? { ...at, top: Math.max(8 - at.height - 8, at.top - over) } : at
    })
  }, [whenOpen])

  // Anchored to where the trigger WAS: once the body scrolls or the window resizes it no longer is, so it folds away.
  useEffect(() => {
    if (!whenOpen) return
    const scroller = body.current
    // The lift in `openWhen` reports its own scroll a frame late; only a scroll away from there moves the trigger.
    const opened = scroller?.scrollTop ?? 0
    const fold = () => whenPicker.current?.close()
    const onScroll = () => {
      if (Math.abs((scroller?.scrollTop ?? 0) - opened) > 2) fold()
    }
    scroller?.addEventListener('scroll', onScroll, { passive: true })
    window.addEventListener('resize', fold)
    return () => {
      scroller?.removeEventListener('scroll', onScroll)
      window.removeEventListener('resize', fold)
    }
  }, [whenOpen])

  function openWhen() {
    const trigger = whenTrigger.current
    const scroller = body.current
    if (!trigger) return
    // A calendar needs room under its trigger: lift the trigger to the body's top first when it has less.
    const room = window.innerHeight - trigger.getBoundingClientRect().bottom
    if (scroller && room < CALENDAR_ROOM) {
      scroller.scrollTop += trigger.getBoundingClientRect().top - scroller.getBoundingClientRect().top - 16
    }
    const rect = trigger.getBoundingClientRect()
    setWhenAnchor({ top: rect.top, left: rect.left, width: rect.width, height: rect.height })
    setWhenOpen(true)
  }
  // Empty means no limit: the sale then covers every listed copy.
  const [cap, setCap] = useState('')
  const capOn = cap !== ''
  const [touched, setTouched] = useState(false)
  const [busy, setBusy] = useState(false)
  // A wallet prompt is open and nothing is signed yet: the creator may still walk away (a dropped WalletConnect
  // session never answers). A result that lands after closing is picked up by the flow around this modal.
  const [awaitingWallet, setAwaitingWallet] = useState(false)
  // Closed while the wallet prompt was open: a late result is the flow's to show, not this unmounted dialog's.
  const mounted = useRef(true)
  useEffect(() => {
    // Set again on every mount: StrictMode unmounts and remounts once in development.
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])
  // A submit whose wallet prompt the creator backed out of: its rejection is ignored, but a signature that still
  // lands creates the discount, so its success is shown.
  const attempt = useRef(0)
  // A submit not yet settled, including one backed out of whose wallet prompt may still be signed.
  const unsettled = useRef(false)
  // The attempt backed out of while still open: Submit shows it again rather than opening a second prompt, which
  // could sign a second coupon, and the terms stay locked to what that prompt signs.
  const backedOut = useRef<number | null>(null)
  const [heldOpen, setHeldOpen] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [created, setCreated] = useState<CollectionDiscount | null>(null)

  const isManaged = isSocialLogin(session)
  const pct = pctPreset === 'custom' ? Number(customPct) : pctPreset

  /** The collection split the way the review reads it: what the sale re-prices, and what it cannot touch. */
  const review = useMemo(() => {
    const listed = current.items.filter(i => i.state === 'discounted')
    const classic = current.items.filter(i => i.state === 'classic')
    const unlisted = current.items.filter(i => i.state === 'unlisted')
    // What the sale can move at most: the cap when there is one, otherwise every remaining copy of every
    // listed item — the honest ceiling for "how many can be sold at this price".
    const supply = listed.reduce((sum, i) => sum + i.remainingSupply, 0)
    return { listed, classic, unlisted, supply }
  }, [current])

  // The terms as they stand, validated the way the submit will validate them, so the button and the inline
  // message agree. `now` is taken per render: a "72 hours" sale is measured from the click, not from mount.
  const terms = useMemo(() => {
    const now = Math.max(Date.now(), clock)
    const { startsAtMs, endsAtMs } = windowOf(when, now)
    const uses = capOn ? (review.supply > 0 ? Math.min(Number(cap), review.supply) : Number(cap)) : undefined
    const candidate = { collections: selected, discountPct: pct, startsAtMs, endsAtMs, uses }
    let problem: DiscountInputProblem | null = null
    try {
      validateDiscountTerms(candidate, now)
    } catch (e) {
      problem = e instanceof DiscountInputError ? e.problem : 'window'
    }
    return { ...candidate, problem }
  }, [selected, pct, when, capOn, cap, review.supply, clock])

  // The page under the scrim stays put while the modal is open, as under every other dialog here.
  useEffect(() => {
    const root = document.documentElement
    const previous = root.style.overflow
    root.style.overflow = 'hidden'
    return () => {
      root.style.overflow = previous
    }
  }, [])

  const blocked = review.listed.length === 0 && review.classic.length > 0

  // Keyboard focus as the shared Modal handles it: in the dialog while it is open (each step swaps the card, and
  // Continue unmounts the very button that was focused), Tab kept inside it, and handed back to whatever opened it.
  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
    return () => opener?.focus()
  }, [])
  useEffect(() => {
    const dialog = card.current
    if (dialog && !dialog.contains(document.activeElement)) dialog.focus()
  }, [step, created, busy])
  useEffect(() => {
    // The calendar floats outside the card and keeps its own focus and Escape while it is open; the blocked
    // notice, the pending state and the success are shared modals, which handle both themselves.
    if (whenOpen || blocked || created || busy) return
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose()
        return
      }
      const dialog = card.current
      if (event.key !== 'Tab' || !dialog) return
      const focusables = dialog.querySelectorAll<HTMLElement>(
        'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
      )
      if (focusables.length === 0) {
        event.preventDefault()
        return
      }
      const first = focusables[0]
      const last = focusables[focusables.length - 1]
      const inside = dialog.contains(document.activeElement)
      if (event.shiftKey && (document.activeElement === first || !inside)) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && (document.activeElement === last || !inside)) {
        event.preventDefault()
        first.focus()
      }
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  })

  /** Every way out goes through here: leaving without a discount is the funnel's drop-off, and from which step. */
  function onClose() {
    // With a wallet prompt still open the discount may yet be created: that is not a drop-off.
    if (!created && !unsettled.current) {
      track(DISCOUNT_ABANDONED_EVENT, {
        collectionId: current.collectionId,
        lastStep: blocked ? 'blocked' : step
      })
    }
    closeModal()
  }

  // What a buyer will see: the collection's priciest listed items, where the cut reads clearest.
  const previewItems = useMemo(
    () =>
      current.items
        .filter(i => i.state === 'discounted' && i.priceCredits != null)
        .sort((a, b) => (b.priceCredits as number) - (a.priceCredits as number)),
    [current]
  )

  /** Whether the strip can scroll either way, so its arrows show only when there is somewhere to go. */
  function measureStrip() {
    const el = strip.current
    if (!el) return
    const start = el.scrollLeft <= 1
    const end = el.scrollLeft + el.clientWidth >= el.scrollWidth - 1
    setStripEdges(prev => (prev.start === start && prev.end === end ? prev : { start, end }))
  }
  function scrollStrip(direction: 1 | -1) {
    const el = strip.current
    if (el) el.scrollBy({ left: direction * el.clientWidth * 0.8, behavior: 'smooth' })
  }
  useEffect(measureStrip, [previewItems, step])

  function releaseBackedOut(run: number) {
    if (backedOut.current !== run) return false
    backedOut.current = null
    setHeldOpen(false)
    return true
  }

  function resume(run: number) {
    releaseBackedOut(run)
    attempt.current = run
    setError(null)
    setBusy(true)
    setAwaitingWallet(true)
  }

  async function submit() {
    if (backedOut.current !== null) return resume(backedOut.current)
    setTouched(true)
    setError(null)
    if (terms.problem) {
      setError(problemCopy(t, terms.problem))
      return
    }
    const signedWindow = reviewed ?? { ...windowOf(when, Date.now()), uses: terms.uses }
    const signedAt = Date.now()
    // A start the review promised that has since gone by would sign a live sale under a "Schedule" button.
    if (signedWindow.startsAtMs !== undefined && signedWindow.startsAtMs <= signedAt) {
      setError(t('creator_sale.error_start_passed'))
      return
    }
    try {
      validateDiscountTerms({ ...terms, ...signedWindow }, signedAt)
    } catch (e) {
      setError(problemCopy(t, e instanceof DiscountInputError ? e.problem : 'window'))
      return
    }
    const run = ++attempt.current
    unsettled.current = true
    setBusy(true)
    try {
      setAwaitingWallet(!isManaged)
      const sale = await onSubmit({ discountPct: terms.discountPct, ...signedWindow }, () => {
        // Signed after backing out: the discount is being created, so show that rather than the review.
        if (backedOut.current === run) resume(run)
        if (attempt.current === run) setAwaitingWallet(false)
      })
      if (!mounted.current) return
      setCreated(sale)
      showToast(sale.status === 'scheduled' ? t('creator_sale.toast_scheduled') : t('creator_sale.toast_live'))
    } catch (e) {
      const reason = toDiscountError(e).reason
      // A backed-out attempt still owns the review until it settles, so its failure is shown there too.
      const backedOutOf = releaseBackedOut(run)
      if (!mounted.current || (attempt.current !== run && !backedOutOf)) return
      if (e instanceof DiscountInputError) setError(problemCopy(t, e.problem))
      // Signed, but whether it was stored is unknown: only that same coupon may be retried, never a new one.
      else if (reason === 'unconfirmed') onUnconfirmed()
      // Dismissing the wallet prompt is the creator changing their mind: back to the review, nothing to report.
      else if (reason !== 'rejected') {
        setError(reason === 'wallet' ? t('wallet_error.wrong_account') : t('creator_sale.error_generic'))
      }
    } finally {
      unsettled.current = false
      releaseBackedOut(run)
      if (attempt.current === run) {
        setBusy(false)
        setAwaitingWallet(false)
      }
    }
  }

  function backOutOfWallet() {
    backedOut.current = attempt.current++
    setHeldOpen(true)
    setBusy(false)
    setAwaitingWallet(false)
  }

  function viewStore() {
    onClose()
    openExternal(shopCreatorUrl(session.address))
  }

  // A percentage the terms refuse is not a price anyone will see, so the preview shows none.
  const previewPct = terms.problem === 'pct' ? 0 : pct
  // Whether any price shown rounds away from the exact cut: Credits are whole, so the badge and the price can disagree.
  const rounded = useMemo(
    // A MANA price is converted at a live rate, so its credits are always rounded.
    () =>
      previewPct > 0 && previewItems.some(i => i.mana || ((i.priceCredits as number) * (100 - previewPct)) % 100 !== 0),
    [previewPct, previewItems]
  )
  const durationPresets = useMemo(
    () =>
      DURATION_PRESETS.map(d => ({
        key: d.key,
        label: durationLabel(t, d.hours),
        active: when.kind === 'preset' && when.key === d.key,
        onPick: () => {
          setTouched(true)
          setWhen({ kind: 'preset', key: d.key, hours: d.hours })
          setWhenOpen(false)
        }
      })),
    [t, when]
  )

  if (created) {
    const scheduled = created.status === 'scheduled'
    return (
      <SuccessModal
        title={scheduled ? t('creator_sale.success_scheduled_title') : t('creator_sale.success_title')}
        description={
          <>
            {t('creator_sale.success_body', { count: created.collections.length, pct: created.discount / 10_000 })}
            <span className={styles.successDetail}>
              {scheduled ? (
                t('creator_sale.success_starts', { date: formatDateTime(created.checks.effective, intl.locale) })
              ) : (
                <>
                  {t('creator_sale.success_ends')}{' '}
                  <SaleCountdown
                    className={styles.successCountdown}
                    until={created.checks.expiration}
                    testId="creator-sale-countdown"
                  />
                </>
              )}
            </span>
          </>
        }
        action={{
          label: t('creator_sale.view_store'),
          onClick: viewStore,
          testId: 'creator-sale-view-store',
          external: true
        }}
        onDone={onClose}
        testId="creator-sale-success"
      />
    )
  }

  /** When the sale runs, in one line — the same two facts the success view repeats afterwards. */
  const bold = (chunks: ReactNode[]) => <b>{chunks}</b>

  /**
   * The currency mark in front of whatever the message tags — an amount, or the currency's own name. The
   * message tags rather than spelling the unit out, so the sentence reads the way the prices above it do
   * and the word it used to carry cannot drift out of step with the currency's name.
   */
  const marked = (chunks: ReactNode[]) => (
    <b className={styles.marked}>
      <CurrencyIcon inline />
      {chunks}
    </b>
  )

  /** How many copies the sale price can cover: the cap when set, otherwise the listed items' own supply. */
  const capCopy =
    (reviewed ?? terms).uses !== undefined
      ? tNode('creator_sale.review_cap', { b: bold, count: (reviewed ?? terms).uses as number })
      : tNode('creator_sale.review_no_cap', { b: bold, count: review.supply })

  const steps = (current: number) => (
    <div className={styles.steps}>
      <StepIndicator
        current={current}
        total={2}
        labels={[t('creator_sale.step_setup'), t('creator_sale.step_review')]}
        testId="creator-sale-steps"
      />
    </div>
  )

  if (busy) {
    return (
      <PendingModal
        label={awaitingWallet ? t('collection_discount.confirm_in_wallet') : t('collection_discount.saving')}
        onCancel={awaitingWallet ? backOutOfWallet : undefined}
        testId="creator-sale-pending"
      />
    )
  }

  if (step === 'review') {
    const shown = reviewed ?? terms
    return (
      <div className={modal.scrim} onClick={onClose} role="presentation">
        <div
          className={styles.card}
          ref={card}
          tabIndex={-1}
          data-testid="creator-sale-review"
          onClick={e => e.stopPropagation()}
          role="dialog"
          aria-modal="true"
          aria-label={t('creator_sale.title')}
        >
          <div className={styles.head}>
            <h2 className={modal.title}>{t('creator_sale.title')}</h2>
            <button className={modal.closeButton} type="button" onClick={onClose} aria-label={t('creator_sale.cancel')}>
              <CloseIcon fontSize="small" />
            </button>
          </div>
          <hr className={styles.divider} />
          {steps(2)}
          <div className={styles.body} ref={body}>
            {/* Three labelled rows sharing one column, so the discount, the start and the end line up as
              the same kind of fact. Each end of the window carries how long until it: a date alone does
              not answer "when does this actually happen", which is what a creator is checking here. */}
            <div className={styles.reviewSummary}>
              <div className={styles.reviewWhenRow} data-testid="creator-sale-review-discount">
                <span className={styles.reviewWhenLabel}>{t('creator_sale.discount')}</span>
                <span className={styles.reviewPct}>
                  <SaleTag pct={pct} testId="creator-sale-review-pct" />
                </span>
              </div>
              <div className={styles.reviewWhenRow} data-testid="creator-sale-review-starts">
                <span className={styles.reviewWhenLabel}>{t('creator_sale.review_starts')}</span>
                <b className={styles.reviewWhenValue}>
                  {shown.startsAtMs
                    ? formatDateTime(shown.startsAtMs, intl.locale)
                    : t('creator_sale.review_starts_now')}
                </b>
                {shown.startsAtMs ? <SaleCountdown className={styles.reviewWhenLeft} until={shown.startsAtMs} /> : null}
              </div>
              <div className={styles.reviewWhenRow} data-testid="creator-sale-review-ends">
                <span className={styles.reviewWhenLabel}>{t('creator_sale.review_ends')}</span>
                <b className={styles.reviewWhenValue}>{formatDateTime(shown.endsAtMs, intl.locale)}</b>
                <SaleCountdown className={styles.reviewWhenLeft} until={shown.endsAtMs} />
              </div>
            </div>

            <div className={styles.reviewGroup}>
              <h3 className={styles.reviewGroupTitle}>
                {t('creator_sale.review_on_sale', { count: review.listed.length })}
              </h3>
              <ul className={styles.reviewList} data-testid="creator-sale-review-items">
                {review.listed.map(i => (
                  <li className={styles.reviewRow} key={i.key} data-testid="creator-sale-review-item">
                    <img className={styles.reviewThumb} src={i.thumbnail} alt="" />
                    <span className={styles.reviewName} data-testid="review-name">
                      {i.name}
                    </span>
                    {/* A MANA listing with no rate yet to quote it in credits shows its own currency instead. */}
                    {i.priceCredits === null && i.manaWei !== undefined ? (
                      <span className={styles.reviewPrices}>
                        <span className={styles.reviewWas} data-testid="review-was">
                          <CurrencyIcon currency="mana" size={14} />
                          {formatMana(i.manaWei)}
                        </span>
                        <span className={styles.reviewNow} data-testid="review-now">
                          <CurrencyIcon currency="mana" size={16} />
                          {formatMana(discountedManaWei(i.manaWei, pct))}
                        </span>
                      </span>
                    ) : (
                      <span className={styles.reviewPrices}>
                        <span className={styles.reviewWas} data-testid="review-was">
                          <CurrencyIcon size={14} />
                          {formatCredits(i.priceCredits as number)}
                        </span>
                        <span className={styles.reviewNow} data-testid="review-now">
                          <CurrencyIcon size={16} />
                          {formatCredits(discountedPriceCredits(i, pct, current.manaRate) as number)}
                        </span>
                      </span>
                    )}
                  </li>
                ))}
              </ul>
            </div>

            {/* Its own section, above the unlisted ones: this is the group the creator can DO something about,
              and the one where "not for sale" would have been a lie. */}
            {review.classic.length > 0 ? (
              <div className={styles.reviewGroup}>
                <h3 className={styles.reviewGroupTitle}>
                  {t('creator_sale.review_classic', { count: review.classic.length })}
                </h3>
                <ul className={styles.reviewList} data-testid="creator-sale-review-classic">
                  {review.classic.map(i => (
                    <li className={styles.reviewRow} key={i.key} data-muted>
                      <img className={styles.reviewThumb} src={i.thumbnail} alt="" />
                      <span className={styles.reviewName}>{i.name}</span>
                      <span className={styles.reviewUnaffected}>
                        {t('creator_sale.review_classic_tag')}
                        <Tooltip content={t('creator_sale.review_classic_why')}>
                          <Icon
                            className={styles.unaffectedInfo}
                            name="info"
                            role="img"
                            aria-label={t('creator_sale.review_classic_why')}
                            tabIndex={0}
                            data-testid="creator-sale-classic-why"
                          />
                        </Tooltip>
                      </span>
                    </li>
                  ))}
                </ul>
                <p className={styles.reviewFootNote}>{tNode('creator_sale.review_classic_hint', { c: marked })}</p>
              </div>
            ) : null}

            {/* Named explicitly rather than left out: an item the creator did not list is untouched by the
              sale, and silence there reads as "everything is covered". */}
            {review.unlisted.length > 0 ? (
              <div className={styles.reviewGroup}>
                <h3 className={styles.reviewGroupTitle}>
                  {t('creator_sale.review_untouched', { count: review.unlisted.length })}
                </h3>
                <ul className={styles.reviewList} data-testid="creator-sale-review-untouched">
                  {review.unlisted.map(i => (
                    <li className={styles.reviewRow} key={i.key} data-muted>
                      <img className={styles.reviewThumb} src={i.thumbnail} alt="" />
                      <span className={styles.reviewName}>{i.name}</span>
                      <span className={styles.reviewUnaffected}>{t('creator_sale.review_not_listed')}</span>
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}

            <p className={styles.reviewFoot}>
              <InfoOutlinedIcon fontSize="small" aria-hidden />
              <span>{capCopy}</span>
            </p>
          </div>

          <div className={styles.actions}>
            {error ? (
              <div className={styles.actionsNote}>
                <ErrorNotice message={error} testId="creator-sale-error" />
              </div>
            ) : heldOpen ? (
              <div className={styles.actionsNote}>
                <p className={styles.reviewFoot} data-testid="creator-sale-request-open">
                  <InfoOutlinedIcon fontSize="small" aria-hidden />
                  <span>{t('creator_sale.request_open')}</span>
                </p>
              </div>
            ) : null}
            <Button
              type="button"
              variant="secondary"
              onClick={() => {
                // A rejected signature left its notice on screen when the creator came back to change
                // something — the message then belonged to an attempt that no longer exists.
                setError(null)
                setStep('form')
              }}
              disabled={busy || heldOpen}
              data-testid="creator-sale-back"
            >
              <ChevronLeftIcon fontSize="small" />
              {t('creator_sale.back')}
            </Button>
            <Button type="button" data-testid="creator-sale-submit" onClick={() => void submit()} disabled={busy}>
              {shown.startsAtMs ? t('creator_sale.submit_scheduled') : t('creator_sale.submit')}
            </Button>
          </div>
        </div>
      </div>
    )
  }

  const inlineProblem = touched && terms.problem ? problemCopy(t, terms.problem) : null
  const customPctOpen = pctPreset === 'custom'
  const dateFormat = new Intl.DateTimeFormat(intl.locale, { month: 'short', day: 'numeric' })
  const whenLabel =
    when.kind === 'preset'
      ? t('creator_sale.when_now', { duration: durationLabel(t, when.hours) })
      : t('creator_sale.when_range', { from: dateFormat.format(when.from), to: dateFormat.format(when.to) })

  /**
   * Nothing here a discount can reach: every listing in this collection is at a price it cannot change.
   *
   * Offered anyway, and answered here. The button used to be absent for these collections, which told the
   * creator nothing — least of all that the fix is one page away.
   */
  if (blocked) {
    return (
      <ConfirmModal
        title={t('creator_sale.blocked_title')}
        icon={<CurrencyIcon size={48} />}
        description={tNode('creator_sale.blocked_body', { c: marked, count: review.classic.length })}
        onClose={onClose}
        confirm={{ label: t('creator_sale.blocked_dismiss'), onClick: onClose, testId: 'creator-sale-blocked-dismiss' }}
        testId="creator-sale-blocked"
      />
    )
  }

  return (
    <div
      className={modal.scrim}
      // The calendar folds on the press, before this click lands (at once with reduced motion, or on a long press):
      // a click that started with it open only ever folds it, never the whole modal.
      onPointerDown={() => {
        calendarOpenOnPress.current = whenOpen
      }}
      onClick={() => {
        const foldedCalendar = calendarOpenOnPress.current || whenOpen
        calendarOpenOnPress.current = false
        if (!foldedCalendar) onClose()
      }}
      role="presentation"
    >
      <div
        className={styles.card}
        ref={card}
        tabIndex={-1}
        data-testid="creator-sale-modal"
        onClick={e => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label={t('creator_sale.title')}
      >
        <div className={styles.head}>
          <h2 className={modal.title}>{t('creator_sale.title')}</h2>
          <button className={modal.closeButton} type="button" onClick={onClose} aria-label={t('creator_sale.cancel')}>
            <CloseIcon fontSize="small" />
          </button>
        </div>
        <hr className={styles.divider} />
        {steps(1)}
        <div className={styles.body} ref={body}>
          <p className={styles.subtitle}>{t('creator_sale.subtitle')}</p>

          {/* Read-only: the sale is scoped to the collection the creator opened it from, so this states the
            context rather than offering a choice. */}
          <div className={styles.collectionRow} data-testid="creator-sale-collections">
            <span className={styles.rowThumb}>
              <CollectionMosaic collectionId={current.collectionId} itemCount={current.itemCount} />
            </span>
            <span className={styles.rowText}>
              <span className={styles.rowName}>{current.name}</span>
              <span className={styles.rowMeta}>
                {t('creator_sale.collection_listed', { count: current.listedCount })}
              </span>
            </span>
          </div>

          <div className={styles.field}>
            <span className={styles.fieldLabel}>{t('creator_sale.discount')}</span>
            <div
              className={styles.chips}
              role="group"
              aria-label={t('creator_sale.discount')}
              data-testid="creator-sale-discounts"
            >
              {PCT_PRESETS.map(p => (
                <button
                  className={styles.chip}
                  key={p}
                  type="button"
                  data-selected={pctPreset === p || undefined}
                  aria-pressed={pctPreset === p}
                  aria-label={t('creator_sale.off_pct', { pct: p })}
                  disabled={busy}
                  onClick={() => {
                    setTouched(true)
                    setPctPreset(p)
                  }}
                  data-testid={`creator-sale-pct-${p}`}
                >
                  {t('creator_sale.pct_tag', { pct: p })}
                </button>
              ))}
              {customPctOpen ? (
                <span
                  className={styles.inlineInput}
                  data-selected
                  aria-invalid={touched && terms.problem === 'pct' ? true : undefined}
                  data-testid="creator-sale-custom-pct-field"
                >
                  <span aria-hidden>-</span>
                  <input
                    type="number"
                    min={MIN_DISCOUNT_PCT}
                    max={MAX_DISCOUNT_PCT}
                    step="1"
                    inputMode="numeric"
                    value={customPct}
                    disabled={busy}
                    autoFocus
                    aria-label={t('creator_sale.pct_label')}
                    onChange={e => {
                      setTouched(true)
                      // Whole percentages of at most two digits, or nothing: anything else keeps what was there, so
                      // "5.5" or "1e1" can never be rewritten into a discount nobody typed.
                      const next = e.target.value
                      if (/^\d{0,2}$/.test(next)) setCustomPct(next)
                    }}
                  />
                  <span aria-hidden>%</span>
                </span>
              ) : (
                <button
                  className={styles.chip}
                  type="button"
                  disabled={busy}
                  onClick={() => {
                    setTouched(true)
                    setPctPreset('custom')
                  }}
                  data-testid="creator-sale-custom-pct-chip"
                >
                  {t('creator_sale.custom_pct')}
                </button>
              )}
            </div>
          </div>

          <div className={styles.field}>
            <span className={styles.fieldLabel}>{t('creator_sale.when')}</span>
            <div className={styles.whenWrap}>
              <button
                className={styles.whenTrigger}
                type="button"
                aria-expanded={whenOpen}
                aria-haspopup="dialog"
                aria-label={t('creator_sale.when_aria', { label: t('creator_sale.when'), value: whenLabel })}
                ref={whenTrigger}
                data-sale-when-trigger=""
                disabled={busy}
                onClick={() => (whenOpen ? whenPicker.current?.close() : openWhen())}
                data-testid="creator-sale-when"
              >
                <Icon name="calendar" size={20} aria-hidden />
                <span>{whenLabel}</span>
                <Icon name="arrow-drop-down" size={24} aria-hidden data-open={whenOpen ? '' : undefined} />
              </button>
              {whenOpen && whenAnchor
                ? createPortal(
                    <div className={styles.whenAnchor} ref={anchor} style={whenAnchor}>
                      <RangePicker
                        stretch
                        handle={whenPicker}
                        label={t('creator_sale.when')}
                        triggerSelector={WHEN_TRIGGER}
                        testId="creator-sale-range"
                        presetTestId="creator-sale-when"
                        from={when.kind === 'range' ? when.from : Date.now()}
                        to={when.kind === 'range' ? when.to : Date.now() + (when.hours - 1) * HOUR_MS}
                        min={startOfDay(Date.now())}
                        max={Date.now() + (MAX_DAYS - 1) * DAY_MS}
                        presets={durationPresets}
                        onApply={(from, to) => {
                          setTouched(true)
                          setWhen({ kind: 'range', from, to })
                          setWhenOpen(false)
                        }}
                        onClose={() => setWhenOpen(false)}
                      />
                    </div>,
                    document.body
                  )
                : null}
            </div>
            {terms.startsAtMs ? (
              <p className={styles.fieldHint} data-testid="creator-sale-when-hint">
                {t('creator_sale.starts_on', { date: formatDateTime(terms.startsAtMs, intl.locale) })}
              </p>
            ) : null}
          </div>

          <div className={styles.field}>
            <label className={styles.capLabel} htmlFor="creator-sale-cap">
              {t('creator_sale.cap_label')}
            </label>
            <div className={styles.capInput} data-disabled={busy || undefined}>
              <input
                id="creator-sale-cap"
                type="text"
                inputMode="numeric"
                autoComplete="off"
                value={cap}
                disabled={busy}
                placeholder={t('creator_sale.cap_placeholder')}
                aria-describedby={review.supply > 0 ? 'creator-sale-cap-hint' : undefined}
                data-testid="creator-sale-cap"
                onChange={e => {
                  const v = e.target.value
                  if (!/^\d{0,7}$/.test(v)) return
                  setTouched(true)
                  setCap(review.supply > 0 && Number(v) > review.supply ? String(review.supply) : v)
                }}
              />
              {review.supply > 0 ? (
                <span aria-hidden data-testid="creator-sale-cap-count">
                  {terms.uses ?? 0}/{review.supply}
                </span>
              ) : null}
            </div>
            {review.supply > 0 ? (
              <p className={styles.capHint} id="creator-sale-cap-hint">
                <Icon name="circle-warning" size={15} aria-hidden />
                <span>{t('creator_sale.cap_hint', { count: review.supply })}</span>
              </p>
            ) : null}
            {/* Not in the Shop's modal: there a social login can't end a discount either, but here it is the only place
              to say so before signing. */}
            {isManaged ? (
              <p className={styles.capHint} data-testid="creator-sale-social-note">
                <Icon name="circle-warning" size={15} aria-hidden />
                <span>{t('creator_sale.social_note')}</span>
              </p>
            ) : null}
          </div>

          {/* The collection as a buyer will meet it: the same tag and struck price the Shop's cards wear. */}
          {previewItems.length > 0 ? (
            <div className={styles.field}>
              <div className={styles.previewHead}>
                <span className={styles.fieldLabel}>
                  {t('creator_sale.preview_title')} · {t('creator_sale.preview_count', { count: previewItems.length })}
                </span>
                {stripEdges.start && stripEdges.end ? null : (
                  <div className={styles.stripArrows}>
                    <button
                      className={styles.stripArrow}
                      type="button"
                      aria-label={t('creator_sale.preview_prev')}
                      disabled={stripEdges.start}
                      onClick={() => scrollStrip(-1)}
                      data-testid="creator-sale-preview-prev"
                    >
                      <Icon name="chevron-down" size={16} aria-hidden style={{ transform: 'rotate(90deg)' }} />
                    </button>
                    <button
                      className={styles.stripArrow}
                      type="button"
                      aria-label={t('creator_sale.preview_next')}
                      disabled={stripEdges.end}
                      onClick={() => scrollStrip(1)}
                      data-testid="creator-sale-preview-next"
                    >
                      <Icon name="chevron-down" size={16} aria-hidden style={{ transform: 'rotate(-90deg)' }} />
                    </button>
                  </div>
                )}
              </div>
              <div
                className={styles.previewStrip}
                ref={strip}
                onScroll={measureStrip}
                data-testid="creator-sale-preview"
              >
                {previewItems.map(item => {
                  const price = item.priceCredits as number
                  const sale = discountedPriceCredits(item, previewPct, current.manaRate) as number
                  return (
                    <div className={styles.previewCard} key={item.key} data-testid="creator-sale-preview-item">
                      <div className={styles.previewMedia}>
                        {item.thumbnail ? <img src={item.thumbnail} alt="" /> : null}
                        {sale < price ? <SaleTag className={styles.previewTag} pct={previewPct} /> : null}
                      </div>
                      <span className={styles.previewName}>{item.name}</span>
                      <span className={styles.previewPrices}>
                        <span className={styles.previewNow} data-testid="creator-sale-preview-now">
                          <CurrencyIcon size={14} inline />
                          {formatCredits(sale)}
                        </span>
                        {sale < price ? (
                          <span className={styles.previewWas} data-testid="creator-sale-preview-was">
                            {formatCredits(price)}
                          </span>
                        ) : null}
                      </span>
                    </div>
                  )
                })}
              </div>
              {rounded ? (
                <p className={styles.fieldHint} data-testid="creator-sale-preview-rounding">
                  {t('creator_sale.preview_rounding')}
                </p>
              ) : null}
            </div>
          ) : null}
        </div>
        <div className={styles.actions}>
          {error || inlineProblem ? (
            <div className={styles.actionsNote}>
              <ErrorNotice message={error ?? inlineProblem} testId="creator-sale-error" />
            </div>
          ) : null}
          <Button type="button" variant="secondary" onClick={onClose} disabled={busy} data-testid="creator-sale-cancel">
            {t('creator_sale.cancel')}
          </Button>
          <Button
            type="button"
            data-testid="creator-sale-continue"
            onClick={() => {
              setTouched(true)
              // Measured from this click, not from when the form last rendered: a same-day range left open past
              // midnight would otherwise reach the review with an end that has already gone by.
              const now = Date.now()
              const window = { ...windowOf(when, now), uses: terms.uses }
              let problem = terms.problem
              if (!problem) {
                try {
                  validateDiscountTerms({ ...terms, ...window }, now)
                } catch (e) {
                  problem = e instanceof DiscountInputError ? e.problem : 'window'
                }
              }
              if (problem) setError(problemCopy(t, problem))
              else {
                setError(null)
                setReviewed(window)
                setStep('review')
                if (!reviewTracked.current) {
                  reviewTracked.current = true
                  track(DISCOUNT_REVIEWED_EVENT, {
                    collectionId: current.collectionId,
                    discountPct: terms.discountPct,
                    durationDays: Math.round((window.endsAtMs - (window.startsAtMs ?? now)) / DAY_MS),
                    scheduled: window.startsAtMs !== undefined,
                    capped: window.uses !== undefined,
                    manaItems: review.listed.filter(i => i.mana).length
                  })
                }
              }
            }}
            disabled={busy || (touched && !!terms.problem)}
          >
            {t('creator_sale.continue')}
            <ChevronRightIcon fontSize="small" />
          </Button>
        </div>
      </div>
    </div>
  )
}
