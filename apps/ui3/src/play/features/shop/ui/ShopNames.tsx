// The Shop's NAMEs section: UNIQUE NAME (a NAME to register, typed in the design's hero, then the banner on
// what a NAME brings) and RESALE NAME (the NAMEs others resell, which the page lists). What the typed NAME
// is (too long, taken, available…), its price and what CLAIM NAME and MAKE OFFER do come in as props; the
// page checks the NAME and draws the nametag over the avatar from the same value.

import { Button, Chip, MaskIcon } from '@hud/design'
import uniqueIcon from '../../../assets/shop/names/unique-name.svg'
import resaleIcon from '../../../assets/shop/names/resale-name.svg'
import creditsIcon from '../../../assets/shop/names/credits-outline.svg'
import warningIcon from '../../../assets/shop/names/warning.svg'
import heroGlowLow from '../../../assets/shop/names/hero-glow-low.svg'
import heroGlowHigh from '../../../assets/shop/names/hero-glow-high.svg'
import heroGlowSpark from '../../../assets/shop/names/hero-glow-spark.svg'
import bannerGlowWide from '../../../assets/shop/names/banner-glow-wide.svg'
import bannerGlowThin from '../../../assets/shop/names/banner-glow-thin.svg'
import bannerGlowTop from '../../../assets/shop/names/banner-glow-top.svg'
import bannerArt from '@hud/assets/backpack/outfits-name-banner.webp'
import { pressedWith, type PressedWith } from './pressedWith'
import styles from './ShopNames.module.css'

/** What a World is, which the banner's "World" opens in the browser. */
const WORLDS_DOCS_URL = 'https://docs.decentraland.org/creator/worlds/about/'

/**
 * What the typed NAME is. `short` and `tooLong` are its length (under 2, over 15); `checking` waits for
 * the registrar; `taken` is someone's, `banned` can't be registered; `error`: the check failed.
 */
export type ShopNameStatus = 'idle' | 'short' | 'tooLong' | 'checking' | 'available' | 'taken' | 'banned' | 'error'

export type ShopNamesTab = 'unique' | 'resale'

export interface ShopNamesProps {
  tab: ShopNamesTab
  onTab: (tab: ShopNamesTab) => void
  /** RESALE NAME's page: the page draws the NAMEs on resale. Absent while the Marketplace's rollout is off: no chip, and the section stays on UNIQUE NAME. */
  resale?: React.ReactNode
  /** The typed NAME. */
  value: string
  /** The NAME as edited, already cleaned and capped. */
  onChange: (value: string) => void
  /** Keeps what a NAME may hold of what was typed or pasted (letters and digits). */
  clean: (raw: string) => string
  /** The most the field holds: one past the limit, so typing past it shows it. */
  maxLength: number
  /** The field's focus, for the nametag's caret. */
  onFocusChange?: (focused: boolean) => void
  /** The fewest letters a NAME has. */
  minLength: number
  /** The limit the counter counts to. */
  limit: number
  /** What the field shows after the NAME while it has the focus: the account's `#` and its last four. */
  suffix?: string | null
  status: ShopNameStatus
  /** The NAME's price in whole Credits, once known. */
  price?: number | null
  onClaim: (input: PressedWith) => void
  /** A taken NAME's offer to its owner, on the Marketplace. Absent while its rollout is off: no MAKE OFFER. */
  onMakeOffer?: () => void
  onRetry: () => void
}

export function ShopNames(props: ShopNamesProps): React.JSX.Element {
  const { onTab, resale } = props
  const offersResale = resale != null
  const tab: ShopNamesTab = offersResale ? props.tab : 'unique'
  return (
    <div className={styles.names}>
      <div className={styles.chips} role="group" aria-label="NAMEs">
        <Chip variant="dark" label="Unique name" icon={<MaskIcon src={uniqueIcon} size={26} />} selected={tab === 'unique'} onClick={() => onTab('unique')} />
        {offersResale ? (
          <Chip variant="dark" label="Resale name" icon={<MaskIcon src={resaleIcon} size={26} />} selected={tab === 'resale'} onClick={() => onTab('resale')} />
        ) : null}
      </div>
      {tab === 'unique' ? (
        <>
          <UniqueName {...props} />
          <NameBanner />
        </>
      ) : (
        resale
      )}
    </div>
  )
}

function UniqueName({ value, onChange, clean, maxLength, onFocusChange, minLength, limit, suffix, status, price, onClaim, onMakeOffer, onRetry }: ShopNamesProps): React.JSX.Element {
  const invalid = status === 'tooLong' || status === 'taken' || status === 'banned'
  // An edit is cleaned on each side of the caret and capped where it was made, as a maxLength field caps it:
  // a paste loses its punctuation, not its letters, and typing into a full field takes nothing and leaves
  // the caret where it was. The caret goes back once the field shows the result (after React's own write).
  const edit = (input: HTMLInputElement): void => {
    const raw = input.value
    const at = input.selectionStart ?? raw.length
    const tail = clean(raw.slice(at))
    const head = clean(raw.slice(0, at)).slice(0, Math.max(0, maxLength - tail.length))
    onChange(head + tail)
    queueMicrotask(() => {
      if (document.activeElement === input) input.setSelectionRange(head.length, head.length)
    })
  }
  return (
    <section className={styles.hero} aria-labelledby="shop-names-title">
      <img className={styles.heroGlowLow} src={heroGlowLow} alt="" />
      <img className={styles.heroGlowHigh} src={heroGlowHigh} alt="" />
      <img className={styles.heroGlowSpark} src={heroGlowSpark} alt="" />
      <img className={styles.heroGlowSpark} src={heroGlowSpark} alt="" />
      <div className={styles.heroContent}>
        <div className={styles.titles}>
          <h2 id="shop-names-title" className={styles.title}>
            Get your unique NAME!
          </h2>
          <p className={styles.subtitle}>Own Your Identity in Decentraland</p>
        </div>
        <div className={styles.claim}>
          <div className={styles.field}>
            <label className={`${styles.outline} ${invalid ? styles.invalid : ''}`.trim()}>
              <input
                className={styles.input}
                aria-label="Your NAME"
                // No maxLength attribute: it would cap the raw text before its punctuation is cleaned out.
                value={value}
                placeholder="Write here"
                spellCheck={false}
                autoComplete="off"
                aria-invalid={invalid || undefined}
                aria-describedby="shop-names-feedback"
                onChange={(e) => edit(e.currentTarget)}
                onFocus={() => onFocusChange?.(true)}
                onBlur={() => onFocusChange?.(false)}
              />
              {suffix != null && (
                <span className={styles.suffix} aria-hidden="true">
                  {suffix}
                </span>
              )}
            </label>
            <Feedback value={value} minLength={minLength} limit={limit} status={status} onMakeOffer={onMakeOffer} onRetry={onRetry} />
          </div>
          <div className={styles.cta}>
            <Button
              size="cta"
              disabled={status !== 'available'}
              aria-label={price != null ? `Claim name for ${price} Credits` : 'Claim name'}
              onClick={(e) => onClaim(pressedWith(e))}
            >
              Claim name
              {price != null && (
                <span className={styles.price}>
                  <img src={creditsIcon} alt="" width={13.26} height={14} />
                  {price}
                </span>
              )}
            </Button>
          </div>
        </div>
      </div>
    </section>
  )
}

/** Under the field: its count while it is typed in, or what is wrong with the NAME. Only the message is a
 *  live region: the count would be announced on every keystroke. */
function Feedback({
  value,
  minLength,
  limit,
  status,
  onMakeOffer,
  onRetry
}: Pick<ShopNamesProps, 'value' | 'minLength' | 'limit' | 'status' | 'onMakeOffer' | 'onRetry'>): React.JSX.Element {
  const count = `${value.length}/${limit}`
  const message =
    status === 'tooLong' ? (
      <>
        {count}
        <img src={warningIcon} alt="" width={15} height={15} />
        Character Limit Reached
      </>
    ) : status === 'taken' ? (
      <>
        <img src={warningIcon} alt="" width={15} height={15} />
        {onMakeOffer ? (
          <span>
            {/* Broken where the design breaks it. */}
            This NAME Is Taken. Choose Another NAME Or Make
            <br />
            An Offer To The Owner.{' '}
            <button type="button" className={styles.makeOffer} onClick={onMakeOffer}>
              MAKE OFFER
            </button>
          </span>
        ) : (
          <span>This NAME Is Taken. Choose Another NAME.</span>
        )}
      </>
    ) : status === 'banned' ? (
      <>
        <img src={warningIcon} alt="" width={15} height={15} />
        This NAME Is Not Available.
      </>
    ) : status === 'error' ? (
      <>
        We couldn’t check that NAME right now.{' '}
        <button type="button" className={styles.makeOffer} onClick={onRetry}>
          TRY AGAIN
        </button>
      </>
    ) : status === 'short' ? (
      `${count} NAMEs need at least ${minLength} characters`
    ) : null
  const tone = status === 'tooLong' ? styles.error : status === 'taken' || status === 'banned' ? `${styles.error} ${styles.taken}` : styles.count
  return (
    <div id="shop-names-feedback" className={styles.feedback}>
      {/* Always in place, so a message arriving in it is announced; empty, it takes no room. */}
      <span role="status" className={message == null ? undefined : tone}>
        {message}
      </span>
      {/* The count shows while the field has the focus (the design's FocusedEditing). */}
      {message == null && <span className={`${styles.count} ${styles.onFocus}`}>{count}</span>}
    </div>
  )
}

/** The banner on what a NAME brings, under the hero: the Backpack's, at the Shop's width. */
function NameBanner(): React.JSX.Element {
  return (
    <section className={styles.banner} aria-labelledby="shop-names-banner">
      <div className={styles.bannerClip}>
        <img className={styles.bannerGlowWide} src={bannerGlowWide} alt="" />
        <img className={styles.bannerGlowThin} src={bannerGlowThin} alt="" />
        <img className={styles.bannerGlowTop} src={bannerGlowTop} alt="" />
        <h2 id="shop-names-banner" className={styles.bannerTitle}>
          Unlock 5 more Outfit slots by getting a NAME!
        </h2>
        <p className={styles.bannerBody}>
          NAMEs are unique Decentraland usernames that come with a{' '}
          <a className={styles.world} href={WORLDS_DOCS_URL} target="_blank" rel="noopener noreferrer">
            World
          </a>
          , the ability to create Communities,
          5 more Outfits slots, and 100 Voting Power in the DAO.
          <br />
          <br />
          Get your own for the full Decentraland experience!
        </p>
      </div>
      <img className={styles.bannerArt} src={bannerArt} alt="" />
    </section>
  )
}
