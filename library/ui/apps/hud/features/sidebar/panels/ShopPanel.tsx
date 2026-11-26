// ShopPanel — what the navigation bar's Shop button opens: the player's Credits with BUY CREDITS,
// a line of text and one banner per Shop section, whose button opens the Shop on that section.
// The bar draws the Shop button, and so this panel, only where the host runs the in-world Shop.
// Nothing here is a stand-in: the amount shows once the caller knows it, and a banner shows only
// when the caller can open its section.

import { withUtm } from '@explorer-core/lib/utm'
import { checkoutSiteUrl } from '@hud-host'
import coin from '../../../assets/sidebar-shop/credits.svg?no-inline'
import emotesArt from '../../../assets/sidebar-shop/banner-emotes.webp'
import wearablesArt from '../../../assets/sidebar-shop/banner-wearables.webp'
import styles from './ShopPanel.module.css'
import { Button, MenuPanel } from '@dcl/one-ui'

/** A Shop section a banner leads to. */
export type ShopPanelSection = 'emotes' | 'wearables'

interface ShopPanelProps {
  /** The player's whole Credits. Undefined while unknown: the coin and the amount are then left out. */
  credits?: number
  /** Opens the Shop on a section. A section with no entry has no banner. */
  explore?: Partial<Record<ShopPanelSection, () => void>>
  /** Called after a press that leaves the panel: BUY CREDITS or a banner's button. */
  onClose?: () => void
}

const BANNERS: { section: ShopPanelSection; title: string; action: string; art: string }[] = [
  { section: 'emotes', title: 'EXPRESS\nWITH STYLE', action: 'EXPLORE EMOTES', art: emotesArt },
  { section: 'wearables', title: 'FIND\nYOUR LOOK', action: 'EXPLORE WEARABLES', art: wearablesArt }
]

/** The amount as the design writes it: whole, with no thousands separator. */
const WHOLE = new Intl.NumberFormat('en-US', { maximumFractionDigits: 0, useGrouping: false })

export function ShopPanel({ credits, explore, onClose }: ShopPanelProps): React.JSX.Element {
  const buyCredits = (): void => {
    // The site whose node the in-world checkout reads its balance from (the host's checkoutSiteUrl).
    window.open(withUtm(checkoutSiteUrl('/shop/credits')), '_blank', 'noopener')
    onClose?.()
  }
  const banners = BANNERS.flatMap((banner) => {
    const open = explore?.[banner.section]
    if (open == null) return []
    return [
      <div key={banner.section} className={`${styles.banner} ${styles[banner.section]}`}>
        <img className={styles.art} src={banner.art} alt="" draggable={false} />
        <h3 className={styles.bannerTitle}>{banner.title}</h3>
        <Button appearance="hud"
          variant="accent"
          size="bare"
          className={styles.cta}
          onClick={() => {
            open()
            onClose?.()
          }}
        >
          {banner.action}
        </Button>
      </div>
    ]
  })
  // The design groups the text with the banner under it, 20px apart; the next banner follows the panel's own gap.
  const [first, ...rest] = banners
  return (
    <MenuPanel label="Shop" title="SHOP" width={400}>
      <div className={styles.balanceBar}>
        {credits != null && (
          <p className={styles.balance}>
            <img className={styles.coin} src={coin} alt="Credits" draggable={false} />
            <span className={styles.amount}>{WHOLE.format(credits)}</span>
          </p>
        )}
        <Button appearance="hud" variant="primary" size="bare" className={styles.buy} onClick={buyCredits}>
          BUY CREDITS
        </Button>
      </div>
      <div className={styles.intro}>
        <p className={styles.introText}>Shop your favorite looks and moves to show off your unique style!</p>
        {first}
      </div>
      {rest}
    </MenuPanel>
  )
}
