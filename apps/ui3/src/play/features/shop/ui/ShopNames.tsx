// The Shop's NAMEs section on UNIQUE NAME: a NAME to register, typed in the design's hero, then the banner
// on what a NAME brings; RESALE NAME beside it leads to the NAMEs for resale. What the typed NAME is (too
// long, taken, available…), its price and what CLAIM NAME, MAKE OFFER and RESALE NAME do come in as props;
// the page checks the NAME and draws the nametag over the avatar from the same value.

import { Button, Chip, MaskIcon } from '../../../design'
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
import bannerArt from '../../../assets/backpack/outfits-name-banner.webp'
import { pressedWith, type PressedWith } from './pressedWith'
import styles from './ShopNames.module.css'

/** What a World is, which the banner's "World" opens in the browser. */
const WORLDS_DOCS_URL = 'https://docs.decentraland.org/creator/worlds/about/'

/**
 * What the typed NAME is. `short` and `tooLong` are its length (under 2, over 15); `checking` waits for
 * the registrar; `taken` is someone's, `banned` can't be registered; `error`: the check failed.
 */
export type ShopNameStatus = 'idle' | 'short' | 'tooLong' | 'checking' | 'available' | 'taken' | 'banned' | 'error'

export interface ShopNamesProps {
  /** RESALE NAME: the NAMEs others resell. */
  onResale: () => void
  /** The typed NAME. */
  value: string
  onChange: (value: string) => void
  /** The field's focus, for the nametag's caret. */
  onFocusChange?: (focused: boolean) => void
  /** The most it takes: one past the limit, so typing past it shows it. */
  maxLength: number
  /** The limit the counter counts to. */
  limit: number
  /** What the field shows after the NAME while it has the focus: the account's `#` and its last four. */
  suffix?: string | null
  status: ShopNameStatus
  /** The NAME's price in whole Credits, once known. */
  price?: number | null
  onClaim: (input: PressedWith) => void
  onMakeOffer: () => void
  onRetry: () => void
}

export function ShopNames(props: ShopNamesProps): React.JSX.Element {
  return (
    <div className={styles.names}>
      <div className={styles.chips} role="group" aria-label="NAMEs">
        <Chip variant="dark" label="Unique name" icon={<MaskIcon src={uniqueIcon} size={26} />} selected />
        <Chip variant="dark" label="Resale name" icon={<MaskIcon src={resaleIcon} size={26} />} onClick={props.onResale} />
      </div>
      <UniqueName {...props} />
      <NameBanner />
    </div>
  )
}

function UniqueName({ value, onChange, onFocusChange, maxLength, limit, suffix, status, price, onClaim, onMakeOffer, onRetry }: ShopNamesProps): React.JSX.Element {
  const invalid = status === 'tooLong' || status === 'taken' || status === 'banned'
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
                value={value}
                maxLength={maxLength}
                placeholder="Write here"
                spellCheck={false}
                autoComplete="off"
                aria-invalid={invalid || undefined}
                aria-describedby="shop-names-feedback"
                onChange={(e) => onChange(e.target.value)}
                onFocus={() => onFocusChange?.(true)}
                onBlur={() => onFocusChange?.(false)}
              />
              {suffix != null && (
                <span className={styles.suffix} aria-hidden="true">
                  {suffix}
                </span>
              )}
            </label>
            <Feedback value={value} limit={limit} status={status} onMakeOffer={onMakeOffer} onRetry={onRetry} />
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

/** Under the field: its count while it is typed in, or what is wrong with the NAME. */
function Feedback({
  value,
  limit,
  status,
  onMakeOffer,
  onRetry
}: Pick<ShopNamesProps, 'value' | 'limit' | 'status' | 'onMakeOffer' | 'onRetry'>): React.JSX.Element {
  const count = `${value.length}/${limit}`
  return (
    <div id="shop-names-feedback" className={styles.feedback} role="status">
      {status === 'tooLong' ? (
        <span className={styles.error}>
          {count}
          <img src={warningIcon} alt="" width={15} height={15} />
          Character Limit Reached
        </span>
      ) : status === 'taken' ? (
        <span className={`${styles.error} ${styles.taken}`}>
          <img src={warningIcon} alt="" width={15} height={15} />
          <span>
            {/* Broken where the design breaks it. */}
            This NAME Is Taken. Choose Another NAME Or Make
            <br />
            An Offer To The Owner.{' '}
            <button type="button" className={styles.makeOffer} onClick={onMakeOffer}>
              MAKE OFFER
            </button>
          </span>
        </span>
      ) : status === 'banned' ? (
        <span className={`${styles.error} ${styles.taken}`}>
          <img src={warningIcon} alt="" width={15} height={15} />
          This NAME Is Not Available.
        </span>
      ) : status === 'error' ? (
        <span className={styles.count}>
          We couldn’t check that NAME right now.{' '}
          <button type="button" className={styles.makeOffer} onClick={onRetry}>
            TRY AGAIN
          </button>
        </span>
      ) : (
        // The count shows while the field has the focus (the design's FocusedEditing), and a NAME too
        // short to register says so.
        <span className={`${styles.count} ${status === 'short' ? '' : styles.onFocus}`.trim()}>{status === 'short' ? `${count} NAMEs need at least 2 characters` : count}</span>
      )}
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
